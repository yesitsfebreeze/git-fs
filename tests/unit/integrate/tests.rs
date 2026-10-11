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
