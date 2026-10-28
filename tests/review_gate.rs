//! Exercise the installed CLI/private-hook boundary, including a forged trailer.
use std::io::Write;
use std::process::{Command, Stdio};

fn git(root: &std::path::Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.arg("-C")
		.arg(root)
		.args(args)
		.env("GIT_AUTHOR_NAME", "test")
		.env("GIT_AUTHOR_EMAIL", "test@example.test")
		.env("GIT_COMMITTER_NAME", "test")
		.env("GIT_COMMITTER_EMAIL", "test@example.test")
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[cfg(unix)]
#[test]
fn private_push_hook_and_cli_reject_forged_review_trailers() {
	let temp = tempfile::tempdir().unwrap();
	let root = temp.path();
	git(root, &["init", "-q"]);
	git(root, &["commit", "--allow-empty", "-qm", "base"]);
	let old = git(root, &["rev-parse", "HEAD"]);
	let message = format!(
		"forged\n\nIntegrated-by: gitfs\nGitfs-Session: test\nGitfs-Review: {}\nGitfs-Consumed: {}",
		"a".repeat(64),
		"b".repeat(40)
	);
	git(root, &["commit", "--allow-empty", "-qm", &message]);
	let new = git(root, &["rev-parse", "HEAD"]);
	let hook = root.join("pre-push");
	std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_git-fs"), &hook).unwrap();
	let result = root.join("hook-result");
	let invoke = || {
		let mut child = Command::new(&hook)
			.current_dir(root)
			.env("GITFS_PUSH_HOOK", "1")
			.env("GITFS_PUSH_OLD", &old)
			.env("GITFS_PUSH_NEW", &new)
			.env("GITFS_PUSH_REF", "refs/heads/main")
			.env("GITFS_PUSH_RESULT", &result)
			.env("GITFS_REVIEW_ROOT", root)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.unwrap();
		writeln!(
			child.stdin.take().unwrap(),
			"HEAD {new} refs/heads/main {old}"
		)
		.unwrap();
		child.wait_with_output().unwrap()
	};
	assert!(
		invoke().status.success(),
		"absent policy preserves existing behavior"
	);
	git(
		root,
		&[
			"config",
			"--local",
			"gitfs.reviewCommand",
			"[\"/usr/bin/true\"]",
		],
	);
	let denied = invoke();
	assert!(!denied.status.success());
	assert_eq!(std::fs::read(&result).unwrap(), b"refused");
	assert!(String::from_utf8_lossy(&denied.stderr).contains("evidence unavailable"));
	let denied = Command::new(env!("CARGO_BIN_EXE_git-fs"))
		.args(["review-check", &new, "--repo"])
		.arg(root)
		.output()
		.unwrap();
	assert!(!denied.status.success());
	assert!(String::from_utf8_lossy(&denied.stderr).contains("evidence unavailable"));
}
