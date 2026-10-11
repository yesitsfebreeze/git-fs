//! Git mode end to end through the tools: each change is one commit on the
//! session branch, the worktree waits for materialize, reads and searches see
//! the branch first, rollback undoes one commit, and local mode or a workspace
//! outside a repository still writes the worktree.

use crate::change_record::{Evidence, Provider, Storage};
use crate::service::{Config, Context, Mode, Service, Touch};
use futures::FutureExt;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn git(root: &Path, args: &[&str]) -> String {
	let output = std::process::Command::new("git")
		.args(args)
		.current_dir(root)
		.env("GIT_AUTHOR_NAME", "fixture")
		.env("GIT_AUTHOR_EMAIL", "fixture@example.test")
		.env("GIT_COMMITTER_NAME", "fixture")
		.env("GIT_COMMITTER_EMAIL", "fixture@example.test")
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap()
}

type Recorded = Arc<Mutex<Vec<Evidence>>>;

fn fixture(mode: Mode, repository: bool) -> (tempfile::TempDir, PathBuf, Arc<Service>, Recorded) {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().canonicalize().unwrap();
	std::fs::write(root.join("a.txt"), "one\n").unwrap();
	if repository {
		git(&root, &["init", "-q"]);
		git(&root, &["add", "a.txt"]);
		git(
			&root,
			&["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"],
		);
	}
	let recorded: Recorded = Arc::default();
	let sink = recorded.clone();
	let touch: Touch = Arc::new(move |_context, evidence| {
		sink.lock().unwrap().push(evidence);
		async { Ok(()) }.boxed()
	});
	let config = Config {
		mode,
		..Config::default()
	};
	(dir, root, Arc::new(Service::new(config, touch)), recorded)
}

async fn tool(service: &Arc<Service>, root: &Path, name: &'static str, input: Value) -> Value {
	static CALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
	let context = Context {
		session: "s1".into(),
		run: "r1".into(),
		call: format!(
			"c{}",
			CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
		),
		cwd: root.to_string_lossy().into_owned(),
	};
	service
		.dispatch(name, json!({"op":"call","context":context,"input":input}))
		.await
}

async fn gitfs(service: &Arc<Service>, root: &Path, input: Value) -> Value {
	let result = crate::overlay::handle(
		service.store.clone(),
		json!({"op":"call","context":{"session":"s1","cwd":root},"input":input}),
	)
	.await
	.unwrap_or_else(|e| json!({"content": e, "error": true}));
	let body = if result["content"].is_object() {
		result["content"].clone()
	} else {
		serde_json::from_str::<Value>(result["content"].as_str().unwrap_or_default())
			.unwrap_or_else(|_| result["content"].clone())
	};
	json!({"error": result["error"], "body": body})
}

fn session_commits(root: &Path) -> usize {
	git(root, &["rev-list", "--count", "HEAD..refs/gitfs/s1"])
		.trim()
		.parse()
		.unwrap()
}

fn has_rg() -> bool {
	std::process::Command::new("rg")
		.arg("--version")
		.output()
		.is_ok()
}

#[tokio::test]
async fn git_mode_commits_each_change_and_leaves_the_worktree_until_materialize() {
	let (_dir, root, service, recorded) = fixture(Mode::Git, true);
	assert_eq!(
		tool(&service, &root, "read", json!({"path":"a.txt"})).await["error"],
		false
	);

	let written = tool(
		&service,
		&root,
		"write",
		json!({"path":"a.txt","text":"two\n"}),
	)
	.await;
	assert_eq!(written["error"], false, "{written}");
	assert!(written["content"]
		.as_str()
		.unwrap()
		.contains("refs/gitfs/s1"));
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
	assert_eq!(git(&root, &["show", "refs/gitfs/s1:a.txt"]), "two\n");
	assert_eq!(session_commits(&root), 1);

	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert!(
		read["content"].as_str().unwrap().contains("1:two"),
		"{read}"
	);
	let edited = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"two","new":"three"}),
	)
	.await;
	assert_eq!(edited["error"], false, "{edited}");
	assert_eq!(
		tool(
			&service,
			&root,
			"write",
			json!({"path":"src/new.txt","text":"fresh three\n"})
		)
		.await["error"],
		false
	);
	assert_eq!(session_commits(&root), 3);
	assert!(!root.join("src/new.txt").exists());
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);

	let evidence = recorded.lock().unwrap().clone();
	assert_eq!(evidence.len(), 3);
	assert!(evidence.iter().all(|e| e.provider == Provider::Gitfs
		&& e.target.storage == Storage::GitfsOverlay
		&& e.overlay_commit.after.is_some()));

	// Another writer moved the branch after this read: the edit is refused.
	service
		.store
		.at(&root)
		.unwrap()
		.write_blob("s1", Path::new("a.txt"), b"other\n", "100644")
		.unwrap();
	let stale = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"three","new":"four"}),
	)
	.await;
	assert_eq!(stale["error"], true, "{stale}");
	assert_eq!(session_commits(&root), 4);

	if has_rg() {
		let glob = tool(&service, &root, "glob", json!({"pattern":"*.txt"})).await;
		let glob: Value = glob["content"].clone();
		assert_eq!(glob["items"], json!(["a.txt", "src/new.txt"]));
		let grep = tool(
			&service,
			&root,
			"grep",
			json!({"pattern":"three|one|other"}),
		)
		.await;
		let grep: Value = grep["content"].clone();
		assert_eq!(
			grep["items"],
			json!([{"path":"a.txt","line":1,"text":"other"},{"path":"src/new.txt","line":1,"text":"fresh three"}])
		);
	}

	let ls = gitfs(&service, &root, json!({"op":"ls"})).await;
	let commits = ls["body"]["commits"].as_array().unwrap().clone();
	assert_eq!(commits.len(), 4);
	let oldest = commits[3]["commit"].as_str().unwrap();
	let refused = gitfs(&service, &root, json!({"op":"rollback","commit":oldest})).await;
	assert_eq!(refused["error"], true, "{refused}");
	assert_eq!(
		gitfs(&service, &root, json!({"op":"rollback","commit":"HEAD"})).await["error"],
		true
	);
	let newest = commits[0]["commit"].as_str().unwrap();
	let undone = gitfs(&service, &root, json!({"op":"rollback","commit":newest})).await;
	assert_eq!(undone["error"], false, "{undone}");
	assert_eq!(git(&root, &["show", "refs/gitfs/s1:a.txt"]), "three\n");
	assert_eq!(session_commits(&root), 5);

	let other = gitfs(&service, &root, json!({"op":"ls","session":"s2"})).await;
	assert_eq!(other["body"]["owned"], json!([]));

	let materialized = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(materialized["error"], false, "{materialized}");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"three\n"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("src/new.txt")).unwrap(),
		"fresh three\n"
	);

	// A path whose commit chain cannot be unwound leaves the session by path:
	// the oldest commit stays refused, `rollback {path}` drops src/new.txt.
	let both = gitfs(
		&service,
		&root,
		json!({"op":"rollback","commit":oldest,"path":"src/new.txt"}),
	)
	.await;
	assert_eq!(both["error"], true, "{both}");
	let dropped = gitfs(
		&service,
		&root,
		json!({"op":"rollback","path":"src/new.txt"}),
	)
	.await;
	assert_eq!(dropped["error"], false, "{dropped}");
	let owned = gitfs(&service, &root, json!({"op":"ls"})).await["body"]["owned"].clone();
	assert!(!owned.to_string().contains("src/new.txt"), "{owned}");
	assert_eq!(session_commits(&root), 6);
	let again = gitfs(
		&service,
		&root,
		json!({"op":"rollback","path":"src/new.txt"}),
	)
	.await;
	assert_eq!(again["error"], true, "{again}");
}

#[tokio::test]
async fn local_mode_and_a_workspace_outside_git_write_the_worktree() {
	for (mode, repository) in [(Mode::Local, true), (Mode::Git, false)] {
		let (_dir, root, service, recorded) = fixture(mode, repository);
		assert_eq!(
			tool(&service, &root, "read", json!({"path":"a.txt"})).await["error"],
			false
		);
		let written = tool(
			&service,
			&root,
			"write",
			json!({"path":"a.txt","text":"two\n"}),
		)
		.await;
		assert_eq!(written["error"], false, "{written}");
		assert_eq!(
			std::fs::read_to_string(root.join("a.txt")).unwrap(),
			"two\n"
		);
		assert_eq!(recorded.lock().unwrap()[0].provider, Provider::Fs);
		if repository {
			assert!(git(&root, &["for-each-ref", "refs/gitfs/"]).is_empty());
		}
	}
}

#[tokio::test]
async fn a_session_overlay_under_a_dot_directory_is_searchable() {
	// The walker and the overlay have to agree about what exists. When the
	// walker hid dot-directories the overlay hid them a second time, so a
	// session could write a memo through its own tools and then fail to find
	// what it had just written.
	if !has_rg() {
		return;
	}
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	let written = tool(
		&service,
		&root,
		"write",
		json!({"path":".config/notes/fresh.md","text":"the overlay marker\n"}),
	)
	.await;
	assert_eq!(written["error"], false, "{written}");
	// Git mode leaves the worktree alone, so only the branch has this file.
	assert!(!root.join(".config/notes/fresh.md").exists());

	let glob = tool(&service, &root, "glob", json!({"pattern":"*.md"})).await;
	let glob: Value = glob["content"].clone();
	assert_eq!(glob["items"], json!([".config/notes/fresh.md"]));

	let grep = tool(&service, &root, "grep", json!({"pattern":"overlay marker"})).await;
	let grep: Value = grep["content"].clone();
	assert_eq!(
		grep["items"],
		json!([{"path":".config/notes/fresh.md","line":1,"text":"the overlay marker"}])
	);
}

#[tokio::test]
async fn a_session_read_respects_a_worktree_deletion() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	std::fs::remove_file(root.join("a.txt")).unwrap();
	let read = tool(&service, &root, "read", json!({"path": "a.txt"})).await;
	assert_eq!(read["error"], true, "{read}");
	assert!(read["content"].as_str().unwrap().contains("not found"));
	let listed = tool(&service, &root, "glob", json!({"pattern":"*.txt"})).await;
	assert_eq!(listed["error"], false, "{listed}");
	let listed: Value = listed["content"].clone();
	assert_eq!(listed["items"], json!([]));
	let recreated = tool(
		&service,
		&root,
		"write",
		json!({"path":"a.txt","text":"replacement"}),
	)
	.await;
	assert_eq!(recreated["error"], true, "{recreated}");
	assert!(!root.join("a.txt").exists());
	// A path trunk never held stays not found.
	let read = tool(&service, &root, "read", json!({"path": "b.txt"})).await;
	assert_eq!(read["error"], true, "{read}");
	assert!(read["content"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn a_write_under_a_submodule_names_the_submodule_and_the_way_round() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	let head = git(&root, &["rev-parse", "HEAD"]);
	git(
		&root,
		&[
			"update-index",
			"--add",
			"--cacheinfo",
			&format!("160000,{},sub", head.trim()),
		],
	);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "gitlink"],
	);
	let written = tool(
		&service,
		&root,
		"write",
		json!({"path":"sub/x.txt","text":"x\n"}),
	)
	.await;
	assert_eq!(written["error"], true, "{written}");
	let message = written["content"].as_str().unwrap();
	assert!(
		message.contains("inside the git submodule `sub`"),
		"{message}"
	);
	assert!(
		!message.contains("both a file and as a directory"),
		"{message}"
	);
}

#[tokio::test]
async fn a_session_reads_local_edits_without_silently_adopting_them() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	// Another writer's unintegrated bytes sit on disk only.
	std::fs::write(root.join("a.txt"), "one\nstale disk row\n").unwrap();
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert!(
		read["content"].as_str().unwrap().contains("stale disk row"),
		"{read}"
	);
	let edited = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(edited["error"], true, "{edited}");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\nstale disk row\n"
	);
}

#[tokio::test]
async fn catchup_through_gitfs_replays_onto_trunk_and_refuses_a_conflict_by_path() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	let wrote = tool(
		&service,
		&root,
		"write",
		json!({"path":"b.txt","text":"mine\n"}),
	)
	.await;
	assert_eq!(wrote["error"], false, "{wrote}");
	std::fs::write(root.join("c.txt"), "trunk\n").unwrap();
	git(&root, &["add", "c.txt"]);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "trunk c"],
	);
	let head = git(&root, &["rev-parse", "HEAD"]).trim().to_owned();
	let caught = gitfs(&service, &root, json!({"op":"catchup"})).await;
	assert_eq!(caught["error"], false, "{caught}");
	assert_eq!(caught["body"]["replayed"], json!(["b.txt"]), "{caught}");
	assert_eq!(
		git(&root, &["merge-base", "HEAD", "refs/gitfs/s1"]).trim(),
		head
	);
	// A conflict: trunk moves the path the session holds to other content.
	std::fs::write(root.join("b.txt"), "theirs\n").unwrap();
	git(&root, &["add", "b.txt"]);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "trunk b"],
	);
	let refused = gitfs(&service, &root, json!({"op":"catchup"})).await;
	assert_eq!(refused["error"], true, "{refused}");
	assert!(
		refused["body"].to_string().contains("trunk changed b.txt"),
		"{refused}"
	);
}

#[tokio::test]
async fn read_identity_exposes_disk_and_session_views() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	std::fs::write(root.join("a.txt"), "human edit\n").unwrap();
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert_eq!(read["revision"]["view"], "disk");
	assert_eq!(read["revision"]["matches_disk"], true);
	assert!(read["revision"].get("commit").is_none());
	assert!(read["content"].as_str().unwrap().contains("human edit"));
	let refused = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(refused["error"], true, "{refused}");
	assert!(refused["content"]
		.as_str()
		.unwrap()
		.contains("disk differs"));
	let refused = tool(
		&service,
		&root,
		"write",
		json!({"path":"a.txt","text":"two\n"}),
	)
	.await;
	assert_eq!(refused["error"], true, "{refused}");
	git(&root, &["add", "a.txt"]);
	git(
		&root,
		&[
			"-c",
			"commit.gpgsign=false",
			"commit",
			"-qm",
			"reconcile disk edit",
		],
	);
	let read = tool(&service, &root, "read", json!({"path":"a.txt","raw":true})).await;
	assert_eq!(read["revision"]["view"], "disk");
	assert_eq!(read["revision"]["matches_disk"], true);
	assert_eq!(read["content"], "human edit\n");
	let edit = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"human edit","new":"two"}),
	)
	.await;
	assert_eq!(edit["error"], false, "{edit}");
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert_eq!(read["revision"]["matches_disk"], false);
	gitfs(&service, &root, json!({"op":"materialize"})).await;
	let read = tool(&service, &root, "read", json!({"path":"a.txt","raw":true})).await;
	assert_eq!(read["revision"]["matches_disk"], true);
	assert_eq!(read["content"], "two\n");
}

#[tokio::test]
async fn fresh_session_reads_materialized_prior_session_changes() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	let edited = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one","new":"updated instructions"}),
	)
	.await;
	assert_eq!(edited["error"], false, "{edited}");
	let materialized = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(materialized["error"], false, "{materialized}");
	assert_eq!(git(&root, &["show", "HEAD:a.txt"]), "one\n");
	let context = Context {
		session: "fresh-session".into(),
		run: "fresh-run".into(),
		call: "read".into(),
		cwd: root.to_string_lossy().into_owned(),
	};
	let read = service
		.dispatch(
			"read",
			json!({"op":"call","context":context,"input":{"path":"a.txt","raw":true}}),
		)
		.await;
	assert_eq!(read["error"], false, "{read}");
	assert_eq!(read["content"], "updated instructions\n");
	assert_eq!(read["revision"]["view"], "disk");
}

#[tokio::test]
async fn reads_and_searches_agree_on_staged_and_unstaged_changes() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	for staged in [false, true] {
		std::fs::write(root.join("a.txt"), "local marker\n").unwrap();
		std::fs::write(root.join("new.txt"), "local marker\n").unwrap();
		if staged {
			git(&root, &["add", "a.txt", "new.txt"]);
		}
		for path in ["a.txt", "new.txt"] {
			let read = tool(&service, &root, "read", json!({"path":path,"raw":true})).await;
			assert_eq!(read["content"], "local marker\n", "{read}");
		}
		for (name, input) in [
			("grep", json!({"pattern":"local marker"})),
			(
				"search",
				json!({"stages":[{"kind":"grep","pattern":"local marker"}]}),
			),
		] {
			let found = tool(&service, &root, name, input).await;
			assert_eq!(found["error"], false, "{found}");
			let found: Value = found["content"].clone();
			assert_eq!(found["items"].as_array().unwrap().len(), 2, "{found}");
		}
	}
	std::fs::write(root.join("a.txt"), "changed after read\n").unwrap();
	let edit = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"local marker","new":"overwrite"}),
	)
	.await;
	assert_eq!(edit["error"], true, "{edit}");
	assert!(edit["content"].as_str().unwrap().contains("changed since"));
}
