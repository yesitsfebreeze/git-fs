//! Exact reviewed-tree checkouts must never masquerade as live-ref snapshots.
//! The public API exercises a run-owned, isolated candidate without writing to
//! the common checkout or staging area.

use gitfs::store::StoreConfig;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
	let result = Command::new("git")
		.args(args)
		.current_dir(root)
		.env("GIT_AUTHOR_NAME", "verification")
		.env("GIT_AUTHOR_EMAIL", "verification@example.invalid")
		.env("GIT_COMMITTER_NAME", "verification")
		.env("GIT_COMMITTER_EMAIL", "verification@example.invalid")
		.output()
		.unwrap();
	assert!(
		result.status.success(),
		"git {args:?} failed: {}",
		String::from_utf8_lossy(&result.stderr)
	);
	String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

fn fixture() -> tempfile::TempDir {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q", "-b", "main"]);
	git(dir.path(), &["config", "user.name", "verification"]);
	git(
		dir.path(),
		&["config", "user.email", "verification@example.invalid"],
	);
	std::fs::write(dir.path().join("base.txt"), b"base\n").unwrap();
	std::fs::write(dir.path().join(".gitignore"), b".git-fs/\n").unwrap();
	git(dir.path(), &["add", "base.txt", ".gitignore"]);
	git(dir.path(), &["commit", "-qm", "base"]);
	dir
}

#[test]
fn candidate_checkout_contains_exact_reviewed_tree_without_touching_shared_index() {
	let dir = fixture();
	let root = dir.path();
	let before_head = git(root, &["rev-parse", "HEAD"]);
	let before_index = git(root, &["write-tree"]);
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let session = "candidate-owner-a";
	let content = b"\x00\xff\r\nraw\x00candidate\n";
	let path = Path::new("binary.bin");
	let executable = Path::new("script.sh");

	// Explicitly establish the session's read baseline before publishing its
	// own edit, independent of any native shared-checkout modification.
	assert_eq!(store.read(session, path).unwrap(), None);
	store.write_blob(session, path, content, "100644").unwrap();
	assert_eq!(store.read(session, executable).unwrap(), None);
	store
		.write_blob(session, executable, b"#!/bin/sh\nexit 0\n", "100755")
		.unwrap();
	let reviewed = store.reviewed_tree(session).unwrap();

	let candidate = store.create_candidate_snapshot(session).unwrap();
	let checkout = Path::new(candidate_field(&candidate, "path"));
	let id = candidate_field(&candidate, "id");
	let capability = candidate_field(&candidate, "capability");
	assert_eq!(candidate_field(&candidate, "tree"), reviewed.tree);
	assert_eq!(git(checkout, &["rev-parse", "HEAD^{tree}"]), reviewed.tree);
	assert_eq!(std::fs::read(checkout.join("binary.bin")).unwrap(), content);
	assert!(
		git(checkout, &["ls-files", "--stage", "--", "script.sh"]).starts_with("100755 "),
		"executable Git mode must survive export"
	);
	assert_eq!(std::fs::read(checkout.join("base.txt")).unwrap(), b"base\n");
	assert_eq!(git(checkout, &["status", "--porcelain"]), "");
	assert_eq!(git(root, &["rev-parse", "HEAD"]), before_head);
	assert_eq!(git(root, &["write-tree"]), before_index);
	assert_eq!(git(root, &["status", "--porcelain"]), "");

	assert!(store
		.release_candidate_snapshot("candidate-owner-b", id, capability)
		.is_err());
	assert!(store
		.release_candidate_snapshot(session, id, &"f".repeat(64))
		.is_err());
	assert!(
		checkout.exists(),
		"foreign release must not remove candidate"
	);

	// A later unrelated native commit cannot retroactively change the pinned
	// candidate, even when trunk moves before a check finishes.
	std::fs::write(root.join("trunk.txt"), b"newer trunk\n").unwrap();
	git(root, &["add", "trunk.txt"]);
	git(root, &["commit", "-qm", "advance trunk"]);
	assert_eq!(git(checkout, &["rev-parse", "HEAD^{tree}"]), reviewed.tree);
	assert_eq!(std::fs::read(checkout.join("binary.bin")).unwrap(), content);

	std::fs::write(checkout.join("foreign-output.log"), b"external test output").unwrap();
	assert!(store
		.release_candidate_snapshot(session, id, capability)
		.is_err());
	assert_eq!(
		std::fs::read(checkout.join("foreign-output.log")).unwrap(),
		b"external test output"
	);
	std::fs::remove_file(checkout.join("foreign-output.log")).unwrap();

	store
		.release_candidate_snapshot(session, id, capability)
		.unwrap();
	assert!(
		!checkout.exists(),
		"release should only remove its own clean checkout"
	);
}

#[test]
fn candidate_requires_reviewed_session_without_side_effects() {
	let dir = fixture();
	let root = dir.path();
	let before_head = git(root, &["rev-parse", "HEAD"]);
	let before_index = git(root, &["write-tree"]);
	let store = StoreConfig::new(".git-fs").at(root).unwrap();

	let error = store
		.create_candidate_snapshot("missing-session")
		.unwrap_err();
	let message = error.to_string();
	assert!(
		message.contains("overlay") || message.contains("session"),
		"candidate must reject sessions without a reviewed overlay: {message}"
	);
	assert_eq!(git(root, &["rev-parse", "HEAD"]), before_head);
	assert_eq!(git(root, &["write-tree"]), before_index);
	assert_eq!(git(root, &["status", "--porcelain"]), "");
}

#[test]
fn candidate_merges_a_later_nonoverlapping_trunk_deletion() {
	let dir = fixture();
	let root = dir.path();
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let session = "deletion-review-owner";
	let rel = Path::new("overlay.txt");
	assert_eq!(store.read(session, rel).unwrap(), None);
	store
		.write_blob(session, rel, b"overlay content\n", "100644")
		.unwrap();

	git(root, &["rm", "--", "base.txt"]);
	git(root, &["commit", "-qm", "delete unrelated trunk file"]);
	let trunk = git(root, &["rev-parse", "HEAD"]);
	let reviewed = store.reviewed_tree(session).unwrap();
	assert_eq!(reviewed.head, trunk);

	let candidate = store.create_candidate_snapshot(session).unwrap();
	let checkout = Path::new(candidate_field(&candidate, "path"));
	assert!(!checkout.join("base.txt").exists());
	assert_eq!(
		std::fs::read(checkout.join("overlay.txt")).unwrap(),
		b"overlay content\n"
	);
	assert_eq!(git(checkout, &["rev-parse", "HEAD^{tree}"]), reviewed.tree);
	assert_eq!(git(root, &["rev-parse", "HEAD"]), trunk);
	assert_eq!(git(root, &["status", "--porcelain"]), "");

	store
		.release_candidate_snapshot(
			session,
			candidate_field(&candidate, "id"),
			candidate_field(&candidate, "capability"),
		)
		.unwrap();
}

#[test]
fn two_concurrent_candidates_have_distinct_release_capabilities() {
	let dir = fixture();
	let store = StoreConfig::new(".git-fs").at(dir.path()).unwrap();
	let session = "shared-session-distinct-runs";
	let path = Path::new("change.txt");
	assert_eq!(store.read(session, path).unwrap(), None);
	store
		.write_blob(session, path, b"shared reviewed content\n", "100644")
		.unwrap();

	let first = store.create_candidate_snapshot(session).unwrap();
	let second = store.create_candidate_snapshot(session).unwrap();
	let first_id = candidate_field(&first, "id");
	let second_id = candidate_field(&second, "id");
	let first_cap = candidate_field(&first, "capability");
	let second_cap = candidate_field(&second, "capability");
	assert_ne!(first_id, second_id);
	assert_ne!(first_cap, second_cap);
	assert_eq!(
		candidate_field(&first, "tree"),
		candidate_field(&second, "tree")
	);
	assert!(store
		.release_candidate_snapshot(session, first_id, second_cap)
		.is_err());

	store
		.release_candidate_snapshot(session, first_id, first_cap)
		.unwrap();
	assert!(Path::new(candidate_field(&second, "path")).exists());
	store
		.release_candidate_snapshot(session, second_id, second_cap)
		.unwrap();
}

#[test]
fn checkout_eol_conversion_refuses_nonidentical_raw_blob_bytes() {
	let dir = fixture();
	let root = dir.path();
	std::fs::write(root.join(".gitattributes"), b"base.txt text eol=crlf\n").unwrap();
	git(root, &["add", ".gitattributes"]);
	git(root, &["commit", "-qm", "declare CRLF conversion"]);
	let head = git(root, &["rev-parse", "HEAD"]);
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let session = "eol-refusal";
	let rel = Path::new("new-file.bin");
	assert_eq!(store.read(session, rel).unwrap(), None);
	store
		.write_blob(session, rel, b"\x00\xff", "100644")
		.unwrap();

	let error = store.create_candidate_snapshot(session).unwrap_err();
	assert!(
		error.contains("transformed") || error.contains("checkout"),
		"candidate must reject byte-changing filters/EOL: {error}"
	);
	assert_eq!(git(root, &["rev-parse", "HEAD"]), head);
	assert_eq!(git(root, &["status", "--porcelain"]), "");
	assert!(!git(root, &["worktree", "list", "--porcelain"]).contains("candidate-"));
}

#[test]
fn candidate_cleanup_rejects_foreign_identifiers() {
	let dir = fixture();
	let root = dir.path();
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let wrong_id = "candidate-ffffffffffff-000000000000-0-0";
	assert!(store
		.release_candidate_snapshot("owner-a", wrong_id, &"f".repeat(64))
		.is_err());
	assert!(store
		.release_candidate_snapshot("owner-b", wrong_id, &"f".repeat(64))
		.is_err());
	assert_eq!(git(root, &["status", "--porcelain"]), "");
}

fn candidate_field<'a>(snapshot: &'a Value, name: &str) -> &'a str {
	snapshot[name]
		.as_str()
		.unwrap_or_else(|| panic!("missing candidate field {name}: {snapshot}"))
}
