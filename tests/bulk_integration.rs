//! Bulk integration must not spawn Git once per file for tree/index reads.
use gitfs::store::StoreConfig;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const CHILD: &str = "GITFS_BULK_INTEGRATION_CHILD";
const TEST: &str = "bulk_review_snapshot_and_sync_use_bounded_git_processes";

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.current_dir(root)
		.args(args)
		.env("GIT_AUTHOR_NAME", "test")
		.env("GIT_AUTHOR_EMAIL", "test@example.invalid")
		.env("GIT_COMMITTER_NAME", "test")
		.env("GIT_COMMITTER_EMAIL", "test@example.invalid")
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn starts(trace: &Path) -> usize {
	std::fs::read_to_string(trace)
		.unwrap_or_default()
		.lines()
		.filter_map(|line| serde_json::from_str::<Value>(line).ok())
		.filter(|event| event["event"] == "start")
		.count()
}

#[test]
fn bulk_review_snapshot_and_sync_use_bounded_git_processes() {
	if std::env::var_os(CHILD).is_none() {
		// Trace only this child: parallel tests cannot change its environment.
		let output = Command::new(std::env::current_exe().unwrap())
			.args(["--exact", TEST, "--nocapture"])
			.env(CHILD, "1")
			.output()
			.unwrap();
		assert!(
			output.status.success(),
			"{}\n{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
		print!("{}", String::from_utf8_lossy(&output.stdout));
		return;
	}
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path();
	git(root, &["init", "-q", "-b", "main"]);
	git(root, &["config", "core.autocrlf", "false"]);
	std::fs::write(root.join(".gitignore"), ".git-fs/\n").unwrap();
	for i in 0..12 {
		std::fs::write(root.join(format!("modify {i}.txt")), "base\n").unwrap();
		std::fs::write(root.join(format!("delete {i}.txt")), "base\n").unwrap();
	}
	std::fs::write(root.join("unrelated"), "base\n").unwrap();
	git(root, &["add", "-A"]);
	git(root, &["commit", "-qm", "base"]);
	git(root, &["checkout", "-qb", "fixture"]);
	for i in 0..12 {
		std::fs::write(
			root.join(format!("modify {i}.txt")),
			format!("changed {i}\n"),
		)
		.unwrap();
		std::fs::remove_file(root.join(format!("delete {i}.txt"))).unwrap();
	}
	for i in 0..24 {
		std::fs::write(
			root.join(format!("added [{i}] ü.txt")),
			format!("added {i}\n"),
		)
		.unwrap();
	}
	git(root, &["add", "-A"]);
	git(root, &["commit", "-qm", "overlay"]);
	let overlay = git(root, &["rev-parse", "HEAD"]);
	git(root, &["checkout", "-q", "main"]);
	git(root, &["update-ref", "refs/gitfs/bulk", &overlay]);
	// Preserve a staged peer file and an unstaged peer edit.
	std::fs::write(root.join("peer staged"), "peer\n").unwrap();
	git(root, &["add", "--", "peer staged"]);
	std::fs::write(root.join("unrelated"), "peer edit\n").unwrap();
	// The actual vault case already has some or all files materialized.
	for i in 0..12 {
		std::fs::write(
			root.join(format!("added [{i}] ü.txt")),
			format!("added {i}\n"),
		)
		.unwrap();
		std::fs::remove_file(root.join(format!("delete {i}.txt"))).unwrap();
	}
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let trace_dir = tempfile::tempdir().unwrap();
	let trace = trace_dir.path().join("git.json");
	std::env::set_var("GIT_TRACE2_EVENT", &trace);
	let start = Instant::now();
	let review = store.reviewed_tree("bulk").unwrap();
	let preview_count = starts(&trace);
	let preview_ms = start.elapsed().as_millis();
	assert_eq!(review.owned.len(), 48);
	let start = Instant::now();
	let candidate = store
		.create_candidate_snapshot_checked(
			"bulk",
			Some(&review.tree),
			Some(&serde_json::to_value(&review).unwrap()),
		)
		.unwrap();
	let snapshot_count = starts(&trace) - preview_count;
	let snapshot_ms = start.elapsed().as_millis();
	let before = starts(&trace);
	let start = Instant::now();
	let commit = store
		.commit_reviewed(
			"bulk",
			&review,
			"bulk\n",
			"test",
			"test@example.invalid",
			&AtomicBool::new(false),
		)
		.unwrap();
	let commit_count = starts(&trace) - before;
	let commit_ms = start.elapsed().as_millis();
	let before = starts(&trace);
	let start = Instant::now();
	let sync = store
		.sync_integrated(&review.head, &commit, &review.owned)
		.unwrap();
	let sync_count = starts(&trace) - before;
	let sync_ms = start.elapsed().as_millis();
	assert_eq!(sync["kept"], serde_json::json!([]), "{sync}");
	assert_eq!(sync["synced"].as_array().unwrap().len(), 48);
	std::env::remove_var("GIT_TRACE2_EVENT");
	for i in 0..12 {
		assert_eq!(
			std::fs::read_to_string(root.join(format!("modify {i}.txt"))).unwrap(),
			format!("changed {i}\n")
		);
		assert!(!root.join(format!("delete {i}.txt")).exists());
	}
	for i in 0..24 {
		assert_eq!(
			std::fs::read_to_string(root.join(format!("added [{i}] ü.txt"))).unwrap(),
			format!("added {i}\n")
		);
	}
	assert_eq!(
		git(root, &["diff", "--cached", "--name-only"]),
		"peer staged"
	);
	assert_eq!(git(root, &["diff", "--name-only"]), "unrelated");
	assert_eq!(
		std::fs::read_to_string(root.join("unrelated")).unwrap(),
		"peer edit\n"
	);
	store
		.release_candidate_snapshot(
			"bulk",
			candidate["id"].as_str().unwrap(),
			candidate["capability"].as_str().unwrap(),
		)
		.unwrap();
	println!("Git processes / milliseconds: preview {preview_count}/{preview_ms}, snapshot {snapshot_count}/{snapshot_ms}, commit {commit_count}/{commit_ms}, sync {sync_count}/{sync_ms}");
	assert!(
		preview_count <= 40,
		"preview launched {preview_count} Git processes for 48 files"
	);
	assert!(
		snapshot_count <= 60,
		"snapshot launched {snapshot_count} Git processes for 48 files"
	);
	assert!(
		commit_count <= 50,
		"commit launched {commit_count} Git processes for 48 files"
	);
	assert!(
		sync_count <= 80,
		"sync launched {sync_count} Git processes for 48 files"
	);
	assert!(
		start.elapsed() < Duration::from_secs(120),
		"bulk sync stalled"
	);
}

#[test]
fn synchronization_waits_once_for_a_foreign_index_writer() {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path();
	git(root, &["init", "-q", "-b", "main"]);
	std::fs::write(root.join(".gitignore"), ".git-fs/\n").unwrap();
	git(root, &["add", "-A"]);
	git(root, &["commit", "-qm", "base"]);
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	for i in 0..8 {
		store
			.write_blob(
				"locked",
				Path::new(&format!("file {i}")),
				b"new\n",
				"100644",
			)
			.unwrap();
	}
	let review = store.reviewed_tree("locked").unwrap();
	let commit = store
		.commit_reviewed(
			"locked",
			&review,
			"locked\n",
			"test",
			"test@example.invalid",
			&AtomicBool::new(false),
		)
		.unwrap();
	let index = std::fs::read(root.join(".git/index")).unwrap();
	let lock = root.join(".git/index.lock");
	std::fs::write(&lock, b"foreign writer").unwrap();
	let started = Instant::now();
	let result = store
		.sync_integrated(&review.head, &commit, &review.owned)
		.unwrap();
	assert!(
		started.elapsed() < Duration::from_secs(8),
		"index-lock wait multiplied by path count"
	);
	assert_eq!(result["synced"], serde_json::json!([]));
	assert_eq!(result["kept"].as_array().unwrap().len(), 8);
	assert_eq!(std::fs::read(&lock).unwrap(), b"foreign writer");
	assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
	for i in 0..8 {
		assert!(!root.join(format!("file {i}")).exists());
	}
	std::fs::remove_file(lock).unwrap();
	let recovered = store
		.sync_integrated(&review.head, &commit, &review.owned)
		.unwrap();
	assert_eq!(recovered["kept"], serde_json::json!([]));
	assert_eq!(git(root, &["status", "--porcelain"]), "");
}
