//! Candidate Git commits must never consume the long-lived MCP process stdin.
use gitfs::store::StoreConfig;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const CHILD_MODE: &str = "GITFS_CANDIDATE_OPEN_STDIN_CHILD";
const CHILD_REPO: &str = "GITFS_CANDIDATE_OPEN_STDIN_REPO";
const TEST_NAME: &str = "candidate_snapshot_finishes_while_parent_stdin_stays_open";

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.args(args)
		.current_dir(root)
		.env("GIT_AUTHOR_NAME", "verification")
		.env("GIT_AUTHOR_EMAIL", "verification@example.invalid")
		.env("GIT_COMMITTER_NAME", "verification")
		.env("GIT_COMMITTER_EMAIL", "verification@example.invalid")
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fixture() -> tempfile::TempDir {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path();
	git(root, &["init", "-q", "-b", "main"]);
	git(root, &["config", "user.name", "verification"]);
	git(
		root,
		&["config", "user.email", "verification@example.invalid"],
	);
	std::fs::write(root.join("base.txt"), b"base\n").unwrap();
	std::fs::write(root.join(".gitignore"), b".git-fs/\n").unwrap();
	git(root, &["add", "base.txt", ".gitignore"]);
	git(root, &["commit", "-qm", "base"]);
	dir
}

#[test]
fn candidate_snapshot_finishes_while_parent_stdin_stays_open() {
	if std::env::var_os(CHILD_MODE).is_some() {
		let root = std::env::var(CHILD_REPO).unwrap();
		let store = StoreConfig::new(".git-fs").at(Path::new(&root)).unwrap();
		let session = "candidate-held-stdin";
		let path = Path::new("proposed.txt");
		assert_eq!(store.read(session, path).unwrap(), None);
		store
			.write_blob(session, path, b"candidate\n", "100644")
			.unwrap();
		let review = store.reviewed_tree(session).unwrap();
		let candidate = store.create_candidate_snapshot(session).unwrap();
		assert_eq!(candidate["tree"].as_str(), Some(review.tree.as_str()));
		let checkout = Path::new(candidate["path"].as_str().unwrap());
		assert_eq!(git(checkout, &["rev-parse", "HEAD^{tree}"]), review.tree);
		assert_eq!(
			git(checkout, &["log", "-1", "--pretty=%s"]),
			"git-fs candidate snapshot"
		);
		assert_eq!(
			std::fs::read(checkout.join("proposed.txt")).unwrap(),
			b"candidate\n"
		);
		store
			.release_candidate_snapshot(
				session,
				candidate["id"].as_str().unwrap(),
				candidate["capability"].as_str().unwrap(),
			)
			.unwrap();
		return;
	}

	let dir = fixture();
	let root = dir.path();
	let head = git(root, &["rev-parse", "HEAD"]);
	let index = git(root, &["write-tree"]);
	let refs = git(root, &["show-ref"]);
	let mut child = Command::new(std::env::current_exe().unwrap())
		.args(["--exact", TEST_NAME, "--nocapture"])
		.env(CHILD_MODE, "1")
		.env(CHILD_REPO, root)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();

	// Deliberately keep child.stdin open: the real MCP transport does too.
	let deadline = Instant::now() + Duration::from_secs(10);
	let status = loop {
		if let Some(status) = child.try_wait().unwrap() {
			break status;
		}
		if Instant::now() >= deadline {
			drop(child.stdin.take());
			let _ = child.kill();
			let _ = child.wait();
			panic!("candidate snapshot blocked waiting for inherited MCP stdin");
		}
		thread::sleep(Duration::from_millis(50));
	};
	drop(child.stdin.take());
	let output = child.wait_with_output().unwrap();
	assert_eq!(status, output.status);
	assert!(
		output.status.success(),
		"candidate subprocess failed:\nstdout: {}\nstderr: {}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
	assert_eq!(git(root, &["write-tree"]), index);
	let after_refs = git(root, &["show-ref"]);
	assert!(after_refs.lines().any(|line| line == refs));
	let additional: Vec<_> = after_refs.lines().filter(|line| *line != refs).collect();
	assert_eq!(additional.len(), 1);
	assert!(additional[0].ends_with(" refs/gitfs/candidate-held-stdin"));
	assert_eq!(git(root, &["status", "--porcelain"]), "");
}
