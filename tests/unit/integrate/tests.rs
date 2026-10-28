use super::*;

#[test]
fn auto_subject_shapes() {
	assert_eq!(crate::integrate::auto_subject(&[]), "gitfs: update");
	assert_eq!(
		crate::integrate::auto_subject(&[('A', "a.txt".to_owned())]),
		"gitfs: a.txt"
	);
	assert_eq!(
		crate::integrate::auto_subject(&[('A', "a".to_owned()), ('M', "b".to_owned())]),
		"gitfs: update 2 files"
	);
}

#[test]
fn secret_subject_is_rejected_and_plain_passes() {
	assert!(has_secret_subject("rotate AKIAIOSFODNN7EXAMPLE"));
	assert!(has_secret_subject(
		"add sk-ant-api123456789012345678901234567890"
	));
	assert!(!has_secret_subject("fix the auth flow"));
}

#[test]
fn gate_prompt_demands_json_only() {
	let p = gate_prompt("+x");
	assert!(p.contains("hold"));
	assert!(p.contains("subject"));
	assert!(p.contains("ONLY with JSON"));
}

#[test]
fn configured_gate_and_author_are_validated_instead_of_defaulting_invalid_types() {
	for value in [
		json!({"gate_model":true}),
		json!({"gate_timeout_ms":0}),
		json!({"author":{"name":"x"}}),
		json!({"author":{"name":"forged\nTrailer: x","email":"x@y"}}),
		json!({"force_secrets":"yes"}),
	] {
		let cfg = Config::from(&value, Arc::new(StoreConfig::new(".kern/gitfs")));
		assert!(identity(&cfg).is_err(), "{value}");
	}
	assert!(identity(&Config::from(
		&Value::Null,
		Arc::new(StoreConfig::new(".kern/gitfs"))
	))
	.is_ok());
}

#[test]
fn active_cancellation_is_exact_bounded_and_drop_cancels_the_blocking_work() {
	let cfg = Config::from(&Value::Null, Arc::new(StoreConfig::new(".kern/gitfs")));
	let args = json!({"context":{"session":"s","run":"r","call":"c","cwd":"/repo"}});
	let key = invocation_key(&args).unwrap();
	let guard = Active::start(&cfg, key.clone()).unwrap();
	assert!(Active::start(&cfg, key.clone()).is_err());
	let mut other = args.clone();
	other["context"]["run"] = json!("different");
	assert_ne!(key, invocation_key(&other).unwrap());
	let flag = guard.cancel.clone();
	drop(guard);
	assert!(flag.load(Ordering::SeqCst));
	assert!(cfg.active.lock().unwrap().is_empty());
	let guards: Vec<_> = (0..128)
		.map(|i| Active::start(&cfg, i.to_string()).unwrap())
		.collect();
	assert!(Active::start(&cfg, "overflow".into()).is_err());
	drop(guards);
	assert!(cfg.active.lock().unwrap().is_empty());
}

#[test]
fn preview_revision_binds_author_required_gate_timeout_subject_and_force() {
	let review = crate::store::ReviewedTree {
		repository: "root".into(),
		branch: "main".into(),
		head: "head".into(),
		index_revision: "index".into(),
		overlay: "overlay".into(),
		tree: "tree".into(),
		owned: vec![('A', "owned".into())],
	};
	let base = Config::from(&json!({}), Arc::new(StoreConfig::new(".kern/gitfs")));
	let tree = preview_tree(&base, &review, "session", "subject", false).unwrap();
	for input in [
		json!({"author":{"name":"Other Author","email":"other@example.test"}}),
		json!({"gate_model":"required"}),
		json!({"gate_timeout_ms":100}),
	] {
		let changed = Config::from(&input, base.store.clone());
		assert_ne!(
			tree,
			preview_tree(&changed, &review, "session", "subject", false).unwrap()
		);
	}
	assert_ne!(
		tree,
		preview_tree(&base, &review, "session", "different", false).unwrap()
	);
	assert_ne!(
		tree,
		preview_tree(&base, &review, "session", "subject", true).unwrap()
	);
}

/// A repository with one commit, and a store over it.
fn repo() -> (tempfile::TempDir, std::path::PathBuf, Config) {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().canonicalize().unwrap();
	let git = |args: &[&str]| {
		let ok = std::process::Command::new("git")
			.arg("-C")
			.arg(&root)
			.args(args)
			.env("GIT_AUTHOR_NAME", "t")
			.env("GIT_AUTHOR_EMAIL", "t@t")
			.env("GIT_COMMITTER_NAME", "t")
			.env("GIT_COMMITTER_EMAIL", "t@t")
			.status()
			.unwrap()
			.success();
		assert!(ok, "git {args:?} failed");
	};
	git(&["init", "-q"]);
	std::fs::write(root.join("README"), "base\n").unwrap();
	git(&["add", "-A"]);
	git(&["commit", "-q", "-m", "init"]);
	let cfg = Config::from(&Value::Null, Arc::new(StoreConfig::new(".kern/gitfs")));
	(dir, root, cfg)
}

fn command_policy(root: &std::path::Path, script: &str, timeout: u64) {
	let path = root.join("reviewer.sh");
	std::fs::write(&path, format!("#!/bin/sh\ncat >/dev/null\n{script}\n")).unwrap();
	for (key, value) in [
		("gitfs.reviewCommand", json!(["/bin/sh", path]).to_string()),
		("gitfs.reviewTimeoutMs", timeout.to_string()),
	] {
		assert!(std::process::Command::new("git")
			.arg("-C")
			.arg(root)
			.args(["config", "--local", key, &value])
			.status()
			.unwrap()
			.success());
	}
}

fn passing_command() -> &'static str {
	"printf '%s' '{\"approved\":true,\"summary\":\"No blocking findings\",\"findings\":[]}'"
}

async fn command_preview(root: &std::path::Path, cfg: &Config) -> Value {
	let store = cfg.store.at(root).unwrap();
	if store.tip("s1").unwrap().is_none() {
		store
			.write_blob("s1", std::path::Path::new("a.txt"), b"mine\n", "100644")
			.unwrap();
	}
	integrate(
		Ctx,
		cfg,
		&store,
		"s1",
		&json!({"message":"reviewed change"}),
		Arc::new(AtomicBool::new(false)),
	)
	.await
	.unwrap()["content"]
		.clone()
}

async fn command_commit(root: &std::path::Path, cfg: &Config, preview: &Value) -> Result<Value> {
	let result = integrate(
		Ctx,
		cfg,
		&cfg.store.at(root).unwrap(),
		"s1",
		&json!({"phase":"commit","message":"reviewed change","expected_revision":preview["revision"]}),
		Arc::new(AtomicBool::new(false)),
	)
	.await?;
	if result["error"] == true {
		Err(result["content"]
			.as_str()
			.unwrap_or("unknown integration error")
			.to_owned())
	} else {
		Ok(result)
	}
}

#[tokio::test]
async fn command_review_passes_exact_candidate_and_requires_receipt_at_push() {
	let (_dir, root, cfg) = repo();
	command_policy(&root, passing_command(), 2000);
	let preview = command_preview(&root, &cfg).await;
	assert!(
		preview["tree"]["command_review"]["files"]
			.as_array()
			.unwrap()
			.len() >= 2
	);
	let parent = cfg.store.at(&root).unwrap().head().unwrap().unwrap();
	let result = command_commit(&root, &cfg, &preview).await.unwrap();
	let commit = result["content"]["committed"].as_str().unwrap();
	crate::review::check_commit(&root, commit).unwrap();
	crate::review::check_range(&root, &parent, commit).unwrap();
	let store = cfg.store.at(&root).unwrap();
	store
		.write_blob(
			"different",
			std::path::Path::new("unreviewed.txt"),
			b"unreviewed\n",
			"100644",
		)
		.unwrap();
	let other = store.reviewed_tree("different").unwrap();
	let body = store
		.git_str(&["show", "-s", "--format=%B", commit])
		.unwrap();
	let forged = store
		.git_str(&["commit-tree", &other.tree, "-p", &parent, "-m", &body])
		.unwrap();
	assert!(crate::review::check_commit(&root, forged.trim())
		.unwrap_err()
		.contains("stale"));
	let receipt_path = crate::store::git_dir(&root)
		.unwrap()
		.join("gitfs-review")
		.join(format!("{}.json", preview["revision"].as_str().unwrap()));
	std::fs::remove_file(receipt_path).unwrap();
	assert!(crate::review::check_commit(&root, commit)
		.unwrap_err()
		.contains("evidence unavailable"));
}

#[tokio::test]
async fn command_review_blocks_timeout_failure_malformed_and_inconsistent_approval() {
	for (script, expected, timeout) in [
		("printf '%s' '{\"approved\":false,\"summary\":\"Failure is reachable\",\"findings\":[{\"id\":\"a\",\"severity\":\"high\",\"path\":\"a.txt\",\"line\":1,\"evidence\":\"Observed failing case\"}]}'", "Failure is reachable", 2000),
		("printf 'not-json'", "malformed JSON", 2000),
		("printf '%s' '{\"approved\":true,\"summary\":\"unsafe\",\"findings\":[{\"id\":\"a\",\"severity\":\"high\",\"path\":\"a.txt\",\"line\":1,\"evidence\":\"Observed failing case\"}]}'", "blocking findings", 2000),
		("echo runner-failed >&2; exit 4", "runner-failed", 2000),
		("yes output-overflow", "exceeds 128 KiB", 2000),
		("sleep 30", "timed out", 50),
	] {
		let (_dir, root, cfg) = repo();
		command_policy(&root, script, timeout);
		let preview = command_preview(&root, &cfg).await;
		let head = cfg.store.at(&root).unwrap().head().unwrap();
		let before = std::time::Instant::now();
		let error = command_commit(&root, &cfg, &preview).await.unwrap_err();
		assert!(error.contains(expected), "{error}");
		assert!(before.elapsed() < std::time::Duration::from_secs(5));
		assert_eq!(cfg.store.at(&root).unwrap().head().unwrap(), head);
	}
}

#[tokio::test]
async fn command_review_invalidates_changed_policy_executable_and_candidate() {
	let (_dir, root, cfg) = repo();
	command_policy(&root, passing_command(), 2000);
	let preview = command_preview(&root, &cfg).await;
	command_policy(&root, passing_command(), 2001);
	assert!(command_commit(&root, &cfg, &preview)
		.await
		.unwrap_err()
		.contains("inputs changed"));
	let preview = command_preview(&root, &cfg).await;
	command_policy(
		&root,
		&format!("# changed code\n{}", passing_command()),
		2001,
	);
	assert!(command_commit(&root, &cfg, &preview)
		.await
		.unwrap_err()
		.contains("inputs changed"));
	command_policy(
		&root,
		&format!(
			"git config --local gitfs.reviewTimeoutMs 2003\n{}",
			passing_command()
		),
		2002,
	);
	let preview = command_preview(&root, &cfg).await;
	assert!(command_commit(&root, &cfg, &preview)
		.await
		.unwrap_err()
		.contains("policy changed during"));
	command_policy(
		&root,
		&format!("git update-ref refs/gitfs/s1 HEAD\n{}", passing_command()),
		2000,
	);
	let preview = command_preview(&root, &cfg).await;
	let head = cfg.store.at(&root).unwrap().head().unwrap();
	assert!(command_commit(&root, &cfg, &preview).await.is_err());
	assert_eq!(cfg.store.at(&root).unwrap().head().unwrap(), head);
}

#[tokio::test]
async fn command_review_caps_held_rounds_and_retains_explicit_reassessment() {
	let (_dir, root, cfg) = repo();
	command_policy(&root, "printf '%s' '{\"approved\":false,\"summary\":\"Needs a different approach\",\"findings\":[]}'", 2000);
	let preview = command_preview(&root, &cfg).await;
	for _ in 0..3 {
		assert!(command_commit(&root, &cfg, &preview)
			.await
			.unwrap_err()
			.contains("Needs a different approach"));
	}
	assert!(command_commit(&root, &cfg, &preview)
		.await
		.unwrap_err()
		.contains("held three rounds"));
	assert!(crate::review::reset_rounds(&root, "s1", "").is_err());
	crate::review::reset_rounds(&root, "s1", "Replace the failed algorithm").unwrap();
	command_policy(&root, passing_command(), 2000);
	let preview = command_preview(&root, &cfg).await;
	let result = command_commit(&root, &cfg, &preview).await.unwrap();
	let commit = result["content"]["committed"].as_str().unwrap();
	crate::review::check_commit(&root, commit).unwrap();
	command_policy(&root, passing_command(), 2001);
	assert!(crate::review::check_commit(&root, commit)
		.unwrap_err()
		.contains("stale"));
}

#[tokio::test]
async fn a_integrate_receipt_names_the_consumed_ref_and_a_replay_commits_nothing_more() {
	let (_dir, root, cfg) = repo();
	let store = cfg.store.at(&root).unwrap();
	store
		.write_blob("s1", std::path::Path::new("a.txt"), b"mine\n", "100644")
		.unwrap();
	let consumed = store.tip("s1").unwrap().unwrap();
	let call = |n: &str, input: Value| json!({"op":"call","context":{"session":"s1","run":"r1","call":n,"cwd":root},"input":input});
	let body = |reply: Value| -> Value {
		assert_eq!(reply["error"], false, "{reply}");
		reply["content"].clone()
	};

	let preview = body(
		handle(
			Ctx,
			&cfg,
			call("c1", json!({"op":"integrate","message":"integrate one"})),
		)
		.await
		.unwrap(),
	);
	let revision = preview["revision"].as_str().unwrap().to_owned();
	let commit = json!({"op":"integrate","phase":"commit","message":"integrate one","expected_revision":revision});

	let first = body(handle(Ctx, &cfg, call("c2", commit.clone())).await.unwrap());
	assert_eq!(first["consumed"], consumed);
	assert_eq!(first["committed"], store.head().unwrap().unwrap());
	let landed = store.head().unwrap().unwrap();
	let depth = store.git_str(&["rev-list", "--count", "HEAD"]).unwrap();

	// The shared checkout followed the integrated path.
	assert_eq!(first["worktree"]["synced"], json!(["a.txt"]));
	assert_eq!(first["worktree"]["kept"], json!([]));

	// The same integrate again: the same receipt, and no second commit.
	let replay = body(handle(Ctx, &cfg, call("c3", commit)).await.unwrap());
	let mut receipt = first.clone();
	receipt.as_object_mut().unwrap().remove("worktree");
	assert_eq!(
		replay, receipt,
		"a replayed integrate answers its first receipt"
	);
	assert_eq!(store.head().unwrap().unwrap(), landed);
	assert_eq!(
		store.git_str(&["rev-list", "--count", "HEAD"]).unwrap(),
		depth
	);

	// Reconcile says whether a reviewed revision landed, and commits nothing.
	let landed_receipt = body(
		handle(
			Ctx,
			&cfg,
			call(
				"c4",
				json!({"op":"integrate","phase":"reconcile","expected_revision":revision}),
			),
		)
		.await
		.unwrap(),
	);
	assert_eq!(landed_receipt["phase"], "reconcile");
	assert_eq!(landed_receipt["integrated"], true);
	assert_eq!(landed_receipt["committed"], landed);
	let unknown = body(
		handle(
			Ctx,
			&cfg,
			call(
				"c5",
				json!({"op":"integrate","phase":"reconcile","expected_revision":"00ff"}),
			),
		)
		.await
		.unwrap(),
	);
	assert_eq!(unknown["integrated"], false, "{unknown}");
	assert_eq!(
		store.git_str(&["rev-list", "--count", "HEAD"]).unwrap(),
		depth
	);
	let bare = handle(
		Ctx,
		&cfg,
		call("c6", json!({"op":"integrate","phase":"reconcile"})),
	)
	.await
	.unwrap();
	assert_eq!(bare["error"], true, "{bare}");
}
