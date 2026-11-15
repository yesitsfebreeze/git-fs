//! Git mode end to end through the tools: each change is one commit on the
//! session branch, the worktree waits for materialize, reads and searches see
//! the branch first, rollback undoes one commit, local mode still writes the
//! worktree, and a workspace outside a repository keeps its branches in the
//! store directory.

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

async fn tool_as(
	service: &Arc<Service>,
	root: &Path,
	session: &str,
	name: &'static str,
	input: Value,
) -> Value {
	static CALL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
	let context = Context {
		session: session.into(),
		run: format!("{session}-run"),
		call: format!(
			"live-{}",
			CALL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
		),
		cwd: root.to_string_lossy().into_owned(),
	};
	service
		.dispatch(name, json!({"op":"call","context":context,"input":input}))
		.await
}

#[tokio::test]
async fn live_peer_reads_committed_edits_without_materialize() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	let before = git(&root, &["rev-parse", "HEAD"]);
	let index = std::fs::read(root.join(".git/index")).unwrap();
	assert_eq!(
		tool_as(&service, &root, "agent-a", "read", json!({"path":"a.txt"})).await["error"],
		false
	);
	let write = tool_as(
		&service,
		&root,
		"agent-a",
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(write["error"], false, "{write}");
	let peer = tool_as(&service, &root, "agent-b", "read", json!({"path":"a.txt"})).await;
	assert_eq!(peer["error"], false, "{peer}");
	assert_eq!(peer["revision"]["view"], "live");
	assert!(peer["content"].as_str().unwrap().contains("two"));
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"two\n"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), before);
	assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
	assert_eq!(git(&root, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn live_nonoverlapping_peer_edits_converge() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	std::fs::write(root.join("a.txt"), "alpha\nbeta\n").unwrap();
	git(&root, &["add", "a.txt"]);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "two lines"],
	);
	for session in ["agent-a", "agent-b"] {
		assert_eq!(
			tool_as(&service, &root, session, "read", json!({"path":"a.txt"})).await["error"],
			false
		);
	}
	let first = tool_as(
		&service,
		&root,
		"agent-a",
		"edit",
		json!({"path":"a.txt","old":"alpha","new":"ALPHA"}),
	)
	.await;
	assert_eq!(first["error"], false, "{first}");
	let second = tool_as(
		&service,
		&root,
		"agent-b",
		"edit",
		json!({"path":"a.txt","old":"beta","new":"BETA"}),
	)
	.await;
	assert_eq!(second["error"], false, "{second}");
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"ALPHA\nBETA\n"
	);
	assert_eq!(
		tool_as(&service, &root, "agent-c", "read", json!({"path":"a.txt"})).await["error"],
		false
	);
}

#[tokio::test]
async fn live_conflicting_edit_preserves_previous_commit() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	for session in ["agent-a", "agent-b"] {
		tool_as(&service, &root, session, "read", json!({"path":"a.txt"})).await;
	}
	let first = tool_as(
		&service,
		&root,
		"agent-a",
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(first["error"], false, "{first}");
	let live = git(&root, &["rev-parse", "refs/gitfs-live/current"]);
	let conflict = tool_as(
		&service,
		&root,
		"agent-b",
		"edit",
		json!({"path":"a.txt","old":"one","new":"three"}),
	)
	.await;
	assert_eq!(conflict["error"], true, "{conflict}");
	assert_eq!(git(&root, &["rev-parse", "refs/gitfs-live/current"]), live);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"two\n"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
}

#[tokio::test]
async fn live_peer_searches_new_files_before_materialization() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	let absent = tool_as(
		&service,
		&root,
		"agent-a",
		"read",
		json!({"path":"new.txt"}),
	)
	.await;
	assert_eq!(absent["error"], true, "{absent}");
	let write = tool_as(
		&service,
		&root,
		"agent-a",
		"write",
		json!({"path":"new.txt","text":"shared-marker\n"}),
	)
	.await;
	assert_eq!(write["error"], false, "{write}");
	assert!(!root.join("new.txt").exists());
	let read = tool_as(
		&service,
		&root,
		"agent-b",
		"read",
		json!({"path":"new.txt"}),
	)
	.await;
	assert_eq!(read["error"], false, "{read}");
	assert_eq!(read["revision"]["view"], "live");
	assert!(read["content"].as_str().unwrap().contains("shared-marker"));
	if has_rg() {
		let glob = tool_as(
			&service,
			&root,
			"agent-b",
			"glob",
			json!({"pattern":"*.txt"}),
		)
		.await;
		assert_eq!(
			glob["content"]["items"],
			json!(["a.txt", "new.txt"]),
			"{glob}"
		);
		let grep = tool_as(
			&service,
			&root,
			"agent-b",
			"grep",
			json!({"pattern":"shared-marker"}),
		)
		.await;
		assert_eq!(
			grep["content"]["items"],
			json!([{"path":"new.txt","line":1,"text":"shared-marker"}]),
			"{grep}"
		);
	}
}

#[tokio::test]
async fn live_delete_hides_a_recreated_untracked_file_from_peers() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	let absent = tool_as(
		&service,
		&root,
		"agent-a",
		"read",
		json!({"path":"new.txt"}),
	)
	.await;
	assert_eq!(absent["error"], true, "{absent}");
	let write = tool_as(
		&service,
		&root,
		"agent-a",
		"write",
		json!({"path":"new.txt","text":"published\n"}),
	)
	.await;
	assert_eq!(write["error"], false, "{write}");
	let deleted = tool_as(
		&service,
		&root,
		"agent-a",
		"delete",
		json!({"path":"new.txt"}),
	)
	.await;
	assert_eq!(deleted["error"], false, "{deleted}");
	std::fs::write(root.join("new.txt"), "foreign-recreation\n").unwrap();
	let peer = tool_as(
		&service,
		&root,
		"agent-b",
		"read",
		json!({"path":"new.txt"}),
	)
	.await;
	assert_eq!(peer["error"], true, "{peer}");
	if has_rg() {
		let glob = tool_as(
			&service,
			&root,
			"agent-b",
			"glob",
			json!({"pattern":"*.txt"}),
		)
		.await;
		assert_eq!(glob["content"]["items"], json!(["a.txt"]), "{glob}");
	}
}

#[tokio::test]
async fn live_rollback_is_a_new_shared_commit_peers_can_read() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	tool_as(&service, &root, "s1", "read", json!({"path":"a.txt"})).await;
	let write = tool_as(
		&service,
		&root,
		"s1",
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(write["error"], false, "{write}");
	let first = git(&root, &["rev-parse", "refs/gitfs-live/current"]);
	let tip = git(&root, &["rev-parse", "refs/gitfs/s1"]);
	let rollback = gitfs(
		&service,
		&root,
		json!({"op":"rollback","commit":tip.trim()}),
	)
	.await;
	assert_eq!(rollback["error"], false, "{rollback}");
	assert_ne!(git(&root, &["rev-parse", "refs/gitfs-live/current"]), first);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"one\n"
	);
	let peer = tool_as(&service, &root, "agent-b", "read", json!({"path":"a.txt"})).await;
	assert_eq!(peer["error"], false, "{peer}");
	assert!(peer["content"].as_str().unwrap().contains("1:one"));
}

#[tokio::test]
async fn live_snapshot_pins_a_clean_worktree_and_refuses_unsafe_release() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	let change = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(change["error"], false, "{change}");
	let revision = git(&root, &["rev-parse", "refs/gitfs-live/current"]);
	let created = gitfs(&service, &root, json!({"op":"snapshot"})).await;
	assert_eq!(created["error"], false, "{created}");
	let snapshot = PathBuf::from(created["body"]["path"].as_str().unwrap());
	let id = created["body"]["id"].as_str().unwrap();
	assert_eq!(created["body"]["revision"], revision.trim());
	assert_eq!(
		std::fs::read_to_string(snapshot.join("a.txt")).unwrap(),
		"two\n"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
	std::fs::write(snapshot.join("a.txt"), "dirty\n").unwrap();
	let denied = gitfs(&service, &root, json!({"op":"snapshot","id":id})).await;
	assert_eq!(denied["error"], true, "{denied}");
	assert!(snapshot.join("a.txt").exists());
	std::fs::write(snapshot.join("a.txt"), "two\n").unwrap();
	let foreign = crate::overlay::handle(
		service.store.clone(),
		json!({"op":"call","context":{"session":"agent-b","cwd":root},"input":{"op":"snapshot","id":id}}),
	)
	.await
	.unwrap();
	assert_eq!(foreign["error"], true, "{foreign}");
	let released = gitfs(&service, &root, json!({"op":"snapshot","id":id})).await;
	assert_eq!(released["error"], false, "{released}");
	assert!(!snapshot.exists());
}

#[tokio::test]
async fn live_reconciles_reviewed_head_without_losing_peer_edits() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	std::fs::write(root.join("b.txt"), "base\n").unwrap();
	git(&root, &["add", "b.txt"]);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "second file"],
	);
	tool_as(&service, &root, "agent-a", "read", json!({"path":"a.txt"})).await;
	let edit = tool_as(
		&service,
		&root,
		"agent-a",
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(edit["error"], false, "{edit}");
	let old_live = git(&root, &["rev-parse", "refs/gitfs-live/current"]);
	std::fs::write(root.join("b.txt"), "reviewed\n").unwrap();
	git(&root, &["add", "b.txt"]);
	git(
		&root,
		&[
			"-c",
			"commit.gpgsign=false",
			"commit",
			"-qm",
			"reviewed change",
		],
	);
	let b = tool_as(&service, &root, "agent-b", "read", json!({"path":"b.txt"})).await;
	assert_eq!(b["error"], false, "{b}");
	assert_eq!(b["content"], "1:reviewed\n");
	let a = tool_as(&service, &root, "agent-b", "read", json!({"path":"a.txt"})).await;
	assert_eq!(a["error"], false, "{a}");
	assert!(a["content"].as_str().unwrap().contains("1:two"));
	assert_ne!(
		git(&root, &["rev-parse", "refs/gitfs-live/current"]),
		old_live
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:b.txt"]),
		"reviewed\n"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"two\n"
	);
}

/// Publishes a.txt "two" live, then commits each of `trunk` to a.txt on HEAD.
async fn live_edit_then_trunk(root: &Path, service: &Arc<Service>, trunk: &[&str]) -> String {
	tool_as(service, root, "agent-a", "read", json!({"path":"a.txt"})).await;
	let edit = tool_as(
		service,
		root,
		"agent-a",
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(edit["error"], false, "{edit}");
	let old_live = git(root, &["rev-parse", "refs/gitfs-live/current"])
		.trim()
		.to_owned();
	for text in trunk {
		std::fs::write(root.join("a.txt"), text).unwrap();
		git(root, &["add", "a.txt"]);
		git(
			root,
			&["-c", "commit.gpgsign=false", "commit", "-qm", "trunk"],
		);
	}
	old_live
}

#[tokio::test]
async fn live_reconcile_takes_trunk_when_it_already_committed_the_live_version() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	live_edit_then_trunk(&root, &service, &["two\n", "three\n"]).await;
	let read = tool_as(&service, &root, "agent-b", "read", json!({"path":"a.txt"})).await;
	assert_eq!(read["error"], false, "{read}");
	assert_eq!(read["content"], "1:three\n");
	let log = git(&root, &["log", "--format=%B", "refs/gitfs-live/current"]);
	assert!(!log.contains("conflicting"), "{log}");
}

#[tokio::test]
async fn live_conflict_keeps_trunk_and_never_blocks_other_paths() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	let old_live = live_edit_then_trunk(&root, &service, &["three\n"]).await;
	let other = tool_as(
		&service,
		&root,
		"agent-b",
		"write",
		json!({"path":"fresh.txt","text":"fresh\n"}),
	)
	.await;
	assert_eq!(other["error"], false, "{other}");
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:a.txt"]),
		"three\n"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:fresh.txt"]),
		"fresh\n"
	);
	assert_eq!(git(&root, &["show", &format!("{old_live}:a.txt")]), "two\n");
	let log = git(&root, &["log", "--format=%B", "refs/gitfs-live/current"]);
	assert!(log.contains(&old_live) && log.contains("a.txt"), "{log}");
}

#[tokio::test]
async fn live_capture_publishes_native_disk_changes_for_peers() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	std::fs::write(root.join("native.txt"), "captured\n").unwrap();
	let captured = gitfs(
		&service,
		&root,
		json!({"op":"capture","paths":["native.txt"]}),
	)
	.await;
	assert_eq!(captured["error"], false, "{captured}");
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:native.txt"]),
		"captured\n"
	);
	let peer = tool_as(
		&service,
		&root,
		"agent-b",
		"read",
		json!({"path":"native.txt"}),
	)
	.await;
	assert_eq!(peer["error"], false, "{peer}");
	assert_eq!(peer["revision"]["view"], "live");
	assert!(peer["content"].as_str().unwrap().contains("captured"));
}

fn has_rg() -> bool {
	std::process::Command::new("rg")
		.arg("--version")
		.output()
		.is_ok()
}

#[tokio::test]
async fn delete_records_a_guarded_tombstone_and_rollback_restores_disk() {
	let (_dir, root, service, recorded) = fixture(Mode::Git, true);
	std::fs::write(root.join("other"), "staged\n").unwrap();
	git(&root, &["add", "other"]);
	std::fs::write(root.join("other"), "unstaged\n").unwrap();
	let index = std::fs::read(root.join(".git/index")).unwrap();
	let head = git(&root, &["rev-parse", "HEAD"]);
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		true
	);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	let deleted = tool(&service, &root, "delete", json!({"path":"a.txt"})).await;
	assert_eq!(deleted["error"], false, "{deleted}");
	assert!(root.join("a.txt").exists());
	assert_eq!(
		git(&root, &["diff", "--name-status", "HEAD", "refs/gitfs/s1"]),
		"D\ta.txt\n"
	);
	assert_eq!(
		tool(&service, &root, "read", json!({"path":"a.txt"})).await["error"],
		true
	);
	let tip = git(&root, &["rev-parse", "refs/gitfs/s1"]);
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		false
	);
	assert_eq!(git(&root, &["rev-parse", "refs/gitfs/s1"]), tip);
	assert_eq!(recorded.lock().unwrap().len(), 1);
	assert_eq!(
		gitfs(&service, &root, json!({"op":"materialize"})).await["error"],
		false
	);
	assert!(!root.join("a.txt").exists());
	assert_eq!(
		gitfs(
			&service,
			&root,
			json!({"op":"rollback","commit":tip.trim()})
		)
		.await["error"],
		false
	);
	assert_eq!(
		gitfs(&service, &root, json!({"op":"materialize"})).await["error"],
		false
	);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
	assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
	assert_eq!(
		std::fs::read_to_string(root.join("other")).unwrap(),
		"unstaged\n"
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
}

#[tokio::test]
async fn delete_cancels_a_materialized_creation_and_protects_recreated_files() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	tool(
		&service,
		&root,
		"write",
		json!({"path":"new.txt","text":"new\n"}),
	)
	.await;
	gitfs(&service, &root, json!({"op":"materialize"})).await;
	let deleted = tool(&service, &root, "delete", json!({"path":"new.txt"})).await;
	assert_eq!(deleted["error"], false, "{deleted}");
	assert_eq!(
		tool(&service, &root, "read", json!({"path":"new.txt"})).await["error"],
		true
	);
	if has_rg() {
		let found = tool(&service, &root, "glob", json!({"pattern":"new.txt"})).await;
		assert_eq!(found["content"]["count"], 0, "{found}");
	}
	gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert!(!root.join("new.txt").exists());
	std::fs::write(root.join("new.txt"), "someone else's recreation\n").unwrap();
	let conflict = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(
		conflict["body"]["conflicts"].as_array().unwrap().len(),
		1,
		"{conflict}"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("new.txt")).unwrap(),
		"someone else's recreation\n"
	);
}

#[tokio::test]
async fn local_delete_requires_a_fresh_read_and_records_absence() {
	let (_dir, root, service, recorded) = fixture(Mode::Local, false);
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		true
	);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	std::fs::write(root.join("a.txt"), "changed\n").unwrap();
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		true
	);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	let deleted = tool(&service, &root, "delete", json!({"path":"a.txt"})).await;
	assert_eq!(deleted["error"], false, "{deleted}");
	assert!(!root.join("a.txt").exists());
	let evidence = recorded.lock().unwrap().clone();
	assert_eq!(evidence.len(), 1);
	assert_eq!(evidence[0].after, crate::change_record::Version::Absent {});
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		false
	);
	std::fs::write(root.join("a.txt"), "recreated\n").unwrap();
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		true
	);
	assert!(root.join("a.txt").exists());
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
			json!([{"path":"a.txt","line":1,"text":"three"},{"path":"src/new.txt","line":1,"text":"fresh three"}])
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
	let cleanup = gitfs(
		&service,
		&root,
		json!({"op":"materialize","paths":["src/new.txt"]}),
	)
	.await;
	assert_eq!(cleanup["error"], false, "{cleanup}");
	assert!(!root.join("src/new.txt").exists());
}

#[tokio::test]
async fn local_mode_writes_the_worktree() {
	for repository in [true, false] {
		let (_dir, root, service, recorded) = fixture(Mode::Local, repository);
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
async fn a_workspace_outside_git_keeps_its_branches_in_the_store_directory() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, false);
	let store_git = |args: &[&str]| {
		let mut all = vec!["--git-dir=.git-fs"];
		all.extend(args);
		git(&root, &all)
	};
	assert_eq!(
		tool(&service, &root, "read", json!({"path":"a.txt"})).await["error"],
		false
	);
	for (path, text) in [("a.txt", "two\n"), ("src/new.txt", "fresh\n")] {
		let written = tool(&service, &root, "write", json!({"path":path,"text":text})).await;
		assert_eq!(written["error"], false, "{written}");
	}
	// The branch lives in the store directory; no `.git`, worktree untouched.
	assert!(!root.join(".git").exists());
	assert_eq!(store_git(&["show", "refs/gitfs/s1:a.txt"]), "two\n");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
	assert!(!root.join("src/new.txt").exists());
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert!(
		read["content"].as_str().unwrap().contains("1:two"),
		"{read}"
	);
	if has_rg() {
		let glob = tool(&service, &root, "glob", json!({"pattern":"*"})).await;
		assert_eq!(
			glob["content"]["items"],
			json!(["a.txt", "src/new.txt"]),
			"{glob}"
		);
	}

	// Both rollbacks work on a branch that starts at a root commit.
	let ls = gitfs(&service, &root, json!({"op":"ls"})).await;
	let commits = ls["body"]["commits"].as_array().unwrap().clone();
	assert_eq!(commits.len(), 2, "{ls}");
	let dropped = gitfs(
		&service,
		&root,
		json!({"op":"rollback","path":"src/new.txt"}),
	)
	.await;
	assert_eq!(dropped["error"], false, "{dropped}");
	let oldest = commits[1]["commit"].as_str().unwrap();
	let undone = gitfs(&service, &root, json!({"op":"rollback","commit":oldest})).await;
	assert_eq!(undone["error"], false, "{undone}");
	let owned = gitfs(&service, &root, json!({"op":"ls"})).await["body"]["owned"].clone();
	assert_eq!(owned, json!([]));
	let cleanup = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(cleanup["error"], false, "{cleanup}");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);

	assert_eq!(
		tool(&service, &root, "read", json!({"path":"a.txt"})).await["error"],
		false
	);
	let written = tool(
		&service,
		&root,
		"write",
		json!({"path":"a.txt","text":"three\n"}),
	)
	.await;
	assert_eq!(written["error"], false, "{written}");
	let materialized = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(materialized["error"], false, "{materialized}");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"three\n"
	);

	// Trunk operations name the missing trunk instead of a missing `.git`.
	let catchup = gitfs(&service, &root, json!({"op":"catchup"})).await;
	assert_eq!(catchup["error"], true, "{catchup}");
	assert!(
		catchup.to_string().contains("trunk has no commit yet"),
		"{catchup}"
	);
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
	assert_eq!(read["revision"]["view"], "live");
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

#[tokio::test]
async fn edit_rebases_onto_a_sibling_materialized_revision_when_the_anchor_is_unique() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert_eq!(read["error"], false, "{read}");
	// Another agent commits its row to its own branch, and Stop materializes it
	// onto the shared worktree before anything integrates it to trunk.
	let sibling = service.store.at(&root).unwrap();
	sibling
		.write_blob("s2", Path::new("a.txt"), b"zero\none\n", "100644")
		.unwrap();
	std::fs::write(root.join("a.txt"), "zero\none\n").unwrap();

	let edited = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one\n","new":"two\n"}),
	)
	.await;
	assert_eq!(edited["error"], false, "{edited}");
	assert_eq!(
		git(&root, &["show", "refs/gitfs/s1:a.txt"]),
		"zero\ntwo\n",
		"the edit lands on the sibling's newer text"
	);
}

#[tokio::test]
async fn edit_still_refuses_an_unknown_hand_edit_on_disk() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	std::fs::write(root.join("a.txt"), "hand edit\n").unwrap();
	let edited = tool(
		&service,
		&root,
		"edit",
		json!({"path":"a.txt","old":"one","new":"two"}),
	)
	.await;
	assert_eq!(edited["error"], true, "{edited}");
	assert!(
		edited["content"]
			.as_str()
			.unwrap_or_default()
			.contains("file changed since it was read"),
		"{edited}"
	);
}

#[tokio::test]
async fn glob_paths_use_forward_slashes_on_every_host() {
	let (_dir, root, service, _recorded) = fixture(Mode::Git, true);
	std::fs::create_dir_all(root.join("nested")).unwrap();
	std::fs::write(root.join("nested/deep.txt"), "x\n").unwrap();
	git(&root, &["add", "nested/deep.txt"]);
	git(
		&root,
		&["-c", "commit.gpgsign=false", "commit", "-qm", "nested"],
	);
	if !has_rg() {
		return;
	}
	let glob = tool(&service, &root, "glob", json!({"pattern":"*.txt"})).await;
	let glob: Value = glob["content"].clone();
	assert!(
		glob["items"]
			.as_array()
			.unwrap()
			.iter()
			.any(|p| p == "nested/deep.txt"),
		"{glob}"
	);
}

#[tokio::test]
async fn explicit_delete_without_materialization_keeps_its_absent_view() {
	let (_dir, root, service, _) = fixture(Mode::Git, true);
	tool(
		&service,
		&root,
		"write",
		json!({"path":"new.txt","text":"session\n"}),
	)
	.await;
	let deleted = tool(&service, &root, "delete", json!({"path":"new.txt"})).await;
	assert_eq!(deleted["error"], false, "{deleted}");
	std::fs::write(root.join("new.txt"), "foreign recreation\n").unwrap();
	assert_eq!(
		tool(&service, &root, "read", json!({"path":"new.txt"})).await["error"],
		true
	);
	let result = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(
		result["body"]["conflicts"].as_array().unwrap().len(),
		1,
		"{result}"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("new.txt")).unwrap(),
		"foreign recreation\n"
	);
}

#[tokio::test]
async fn overlay_rollback_supersedes_older_explicit_deletion_of_untouched_disk() {
	let (_dir, root, service, _) = fixture(Mode::Git, false);
	tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	tool(
		&service,
		&root,
		"write",
		json!({"path":"a.txt","text":"session\n"}),
	)
	.await;
	assert_eq!(
		tool(&service, &root, "delete", json!({"path":"a.txt"})).await["error"],
		false
	);
	assert_eq!(
		tool(
			&service,
			&root,
			"write",
			json!({"path":"a.txt","text":"later\n"})
		)
		.await["error"],
		false
	);
	let rolled = gitfs(&service, &root, json!({"op":"rollback","path":"a.txt"})).await;
	assert_eq!(rolled["error"], false, "{rolled}");
	let read = tool(&service, &root, "read", json!({"path":"a.txt"})).await;
	assert_eq!(read["error"], false, "{read}");
	assert!(read["content"].as_str().unwrap().contains("1:one"));
	let cleanup = gitfs(&service, &root, json!({"op":"materialize"})).await;
	assert_eq!(cleanup["error"], false, "{cleanup}");
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"one\n"
	);
}
