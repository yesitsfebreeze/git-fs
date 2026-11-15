//! A verification pass belongs to one reviewed candidate and one configured
//! trusted runner policy, never merely to a session name or command string.

use gitfs::store::StoreConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

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
		"git {args:?} failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn digest(bytes: &[u8]) -> String {
	format!("{:x}", Sha256::digest(bytes))
}

fn policy(receipt_dir: &Path) -> Value {
	json!({
		"schema": "gitfs-verification-policy/v1",
		"runner_id": "overwatch-fixture",
		"receipt_dir": receipt_dir.to_string_lossy(),
		"checks": [{"id": "unit", "definition": "cargo test --locked", "required": true}],
		"environment_allowlist": [],
		"reuse": "exact-candidate",
	})
}

fn preview(config: &Value, session: &str, source: &Value) -> Value {
	json!({
		"schema": "gitfs-reviewed-commit/v1",
		"source": source,
		"session": session,
		"subject": "candidate commit",
		"author": {"name": "verification", "email": "verification@example.invalid",
			"committer": {"name": "kern gitfs", "email": "gitfs@kern.local"}},
		"force_secrets": false,
		"required_gate": null,
		"gate_timeout_ms": 30000,
		"verification_policy": {
			"schema": "gitfs-verification-policy/v1",
			"policy_digest": digest(&serde_json::to_vec(config).unwrap()),
			"check_definition_digest": digest(&serde_json::to_vec(&config["checks"]).unwrap()),
			"runner_id": "overwatch-fixture",
			"reuse": "exact-candidate"
		}
	})
}

fn revision_for(config: &Value, session: &str, source: &Value) -> String {
	digest(&serde_json::to_vec(&preview(config, session, source)).unwrap())
}

fn set_policy(root: &Path, config: &Value) {
	let serialized = serde_json::to_string(config).unwrap();
	git(
		root,
		&["config", "--local", "gitfs.verificationPolicy", &serialized],
	);
}

fn fixture() -> (tempfile::TempDir, tempfile::TempDir) {
	let repo = tempfile::tempdir().unwrap();
	let receipts = tempfile::tempdir().unwrap();
	git(repo.path(), &["init", "-q", "-b", "main"]);
	std::fs::write(repo.path().join(".gitignore"), b".git-fs/\n").unwrap();
	std::fs::write(repo.path().join("base.txt"), b"base\n").unwrap();
	git(repo.path(), &["add", ".gitignore", "base.txt"]);
	git(repo.path(), &["commit", "-qm", "initial"]);
	(repo, receipts)
}

fn source(root: &Path, session: &str, name: &str) -> Value {
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let path = Path::new(name);
	assert!(store.read(session, path).unwrap().is_none());
	store
		.write_blob(session, path, b"candidate\n", "100644")
		.unwrap();
	serde_json::to_value(store.reviewed_tree(session).unwrap()).unwrap()
}

fn receipt(config: &Value, session: &str, source: &Value, revision: &str, dir: &Path) -> Value {
	// Each candidate/run owns distinct artifacts; another runner cannot
	// invalidate its evidence by writing the same log filename.
	let stdout = dir.join(format!("{revision}.stdout"));
	let stderr = dir.join(format!("{revision}.stderr"));
	std::fs::write(&stdout, b"test result: ok\n").unwrap();
	std::fs::write(&stderr, b"").unwrap();
	json!({
		"schema": "gitfs-verification-receipt/v1",
		"run_id": "run-unique-001",
		"session": session,
		"preview": preview(config, session, source),
		"runner": {"id": "overwatch-fixture", "revision": "runner-build-1"},
		"source": source,
		"review_revision": revision,
		"policy_digest": digest(&serde_json::to_vec(config).unwrap()),
		"check_definition_digest": digest(&serde_json::to_vec(&config["checks"]).unwrap()),
		"platform": {"os": std::env::consts::OS, "arch": std::env::consts::ARCH},
		"toolchain": {"rustc": "fixture-toolchain-1"},
		"dependencies": {"lockfile": "fixture-lockfile-sha256"},
		"environment": {"allowlisted": {}},
		"checks": [{
			"id": "unit", "status": "passed", "input_integrity": "unchanged",
			"definition_digest": digest(config["checks"][0]["definition"].as_str().unwrap().as_bytes()),
			"stdout": {"path": stdout.to_string_lossy(), "sha256": digest(b"test result: ok\n")},
			"stderr": {"path": stderr.to_string_lossy(), "sha256": digest(b"")},
			"output_digests": [],
		}],
		"limitations": ["fixture runner, not a hermetic sandbox"],
	})
}

fn store_receipt(dir: &Path, revision: &str, record: &Value) -> PathBuf {
	let path = dir.join(format!("{revision}.json"));
	std::fs::write(&path, serde_json::to_vec(record).unwrap()).unwrap();
	path
}

fn integrated_commit(
	root: &Path,
	source: &Value,
	session: &str,
	revision: &str,
	subject: &str,
	author: &str,
) -> String {
	let message = format!(
        "{subject}\n\nIntegrated-by: gitfs\nGitfs-Session: {session}\nGitfs-Review: {revision}\nGitfs-Consumed: {}\n",
        source["overlay"].as_str().unwrap());
	let output = Command::new("git")
		.arg("-C")
		.arg(root)
		.args([
			"commit-tree",
			source["tree"].as_str().unwrap(),
			"-p",
			source["head"].as_str().unwrap(),
			"-m",
			&message,
		])
		.env("GIT_AUTHOR_NAME", author)
		.env("GIT_AUTHOR_EMAIL", "verification@example.invalid")
		.env("GIT_COMMITTER_NAME", "kern gitfs")
		.env("GIT_COMMITTER_EMAIL", "gitfs@kern.local")
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn configured_policy_requires_a_trusted_store_receipt_and_validates_exact_candidate() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let first = source(root, "same-session", "first.txt");
	let revision = revision_for(&trusted, "same-session", &first);

	// An untrusted tool call cannot simply claim its command passed.
	assert!(
		gitfs::verification::validate_candidate(root, "same-session", &first, &revision).is_err()
	);
	let pass = receipt(&trusted, "same-session", &first, &revision, receipts.path());
	store_receipt(receipts.path(), &revision, &pass);
	assert!(
		gitfs::verification::validate_candidate(root, "same-session", &first, &revision)
			.unwrap()
			.is_some()
	);

	// Same session/runner/command is not an identity: the reviewed tree is.
	let second = source(root, "same-session", "second.txt");
	assert_ne!(first["tree"], second["tree"]);
	assert!(
		gitfs::verification::validate_candidate(root, "same-session", &second, &revision).is_err()
	);
	assert!(
		gitfs::verification::validate_candidate(root, "wrong-session", &first, &revision).is_err()
	);
}

#[test]
fn changed_check_policy_invalidates_earlier_success() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let original = policy(receipts.path());
	set_policy(root, &original);
	let candidate = source(root, "policy-owner", "first.txt");
	let revision = revision_for(&original, "policy-owner", &candidate);
	store_receipt(
		receipts.path(),
		&revision,
		&receipt(
			&original,
			"policy-owner",
			&candidate,
			&revision,
			receipts.path(),
		),
	);
	assert!(
		gitfs::verification::validate_candidate(root, "policy-owner", &candidate, &revision)
			.unwrap()
			.is_some()
	);

	let mut changed = original.clone();
	changed["checks"][0]["definition"] = json!("cargo test --locked --all-targets");
	set_policy(root, &changed);
	assert!(
		gitfs::verification::validate_candidate(root, "policy-owner", &candidate, &revision)
			.is_err()
	);
}

#[test]
fn failed_cancelled_incomplete_or_forged_evidence_never_authorizes_publication() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let candidate = source(root, "failure-owner", "first.txt");
	let revision = revision_for(&trusted, "failure-owner", &candidate);
	let pass = receipt(
		&trusted,
		"failure-owner",
		&candidate,
		&revision,
		receipts.path(),
	);

	for status in ["failed", "cancelled", "timed_out", "running", "unknown"] {
		let mut record = pass.clone();
		record["checks"][0]["status"] = json!(status);
		store_receipt(receipts.path(), &revision, &record);
		assert!(
			gitfs::verification::validate_candidate(root, "failure-owner", &candidate, &revision)
				.is_err(),
			"{status}"
		);
	}
	for field in [
		"runner",
		"run_id",
		"session",
		"policy_digest",
		"toolchain",
		"limitations",
	] {
		let mut record = pass.clone();
		record.as_object_mut().unwrap().remove(field);
		store_receipt(receipts.path(), &revision, &record);
		assert!(
			gitfs::verification::validate_candidate(root, "failure-owner", &candidate, &revision)
				.is_err(),
			"missing {field}"
		);
	}
	let mut wrong_runner = pass.clone();
	wrong_runner["runner"]["id"] = json!("model-supplied-runner");
	store_receipt(receipts.path(), &revision, &wrong_runner);
	assert!(
		gitfs::verification::validate_candidate(root, "failure-owner", &candidate, &revision)
			.is_err()
	);
	let mut wrong_command = pass;
	wrong_command["checks"][0]["definition_digest"] = json!(digest(b"different command"));
	store_receipt(receipts.path(), &revision, &wrong_command);
	assert!(
		gitfs::verification::validate_candidate(root, "failure-owner", &candidate, &revision)
			.is_err()
	);
}

#[test]
fn tampered_logs_or_mutated_input_make_a_prior_pass_ineligible() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let candidate = source(root, "log-owner", "first.txt");
	let revision = revision_for(&trusted, "log-owner", &candidate);
	let record = receipt(
		&trusted,
		"log-owner",
		&candidate,
		&revision,
		receipts.path(),
	);
	store_receipt(receipts.path(), &revision, &record);
	assert!(
		gitfs::verification::validate_candidate(root, "log-owner", &candidate, &revision)
			.unwrap()
			.is_some()
	);

	std::fs::write(
		receipts.path().join(format!("{revision}.stdout")),
		b"later corrupted\n",
	)
	.unwrap();
	assert!(
		gitfs::verification::validate_candidate(root, "log-owner", &candidate, &revision).is_err()
	);
	std::fs::write(
		receipts.path().join(format!("{revision}.stdout")),
		b"test result: ok\n",
	)
	.unwrap();
	let mut mutated = record.clone();
	mutated["checks"][0]["input_integrity"] = json!("changed");
	store_receipt(receipts.path(), &revision, &mutated);
	assert!(
		gitfs::verification::validate_candidate(root, "log-owner", &candidate, &revision).is_err()
	);
}

#[test]
fn no_configured_verification_policy_preserves_existing_unverified_workflows() {
	let (repo, _receipts) = fixture();
	let candidate = source(repo.path(), "unconfigured-owner", "first.txt");
	assert!(gitfs::verification::validate_candidate(
		repo.path(),
		"unconfigured-owner",
		&candidate,
		&"e".repeat(64)
	)
	.unwrap()
	.is_none());
}

#[test]
fn forged_repository_branch_or_head_is_not_a_live_candidate() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let original = source(root, "identity-owner", "first.txt");
	for (field, replacement) in [
		("repository", "foreign-repository"),
		("branch", "foreign-branch"),
		("head", "0000000000000000000000000000000000000000"),
	] {
		let mut forged = original.clone();
		forged[field] = json!(replacement);
		let revision = revision_for(&trusted, "identity-owner", &forged);
		store_receipt(
			receipts.path(),
			&revision,
			&receipt(
				&trusted,
				"identity-owner",
				&forged,
				&revision,
				receipts.path(),
			),
		);
		assert!(
			gitfs::verification::validate_candidate(root, "identity-owner", &forged, &revision)
				.is_err(),
			"forged {field}"
		);
	}
}

#[test]
fn real_head_advance_invalidates_an_earlier_successful_candidate() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let candidate = source(root, "advance-owner", "first.txt");
	let revision = revision_for(&trusted, "advance-owner", &candidate);
	store_receipt(
		receipts.path(),
		&revision,
		&receipt(
			&trusted,
			"advance-owner",
			&candidate,
			&revision,
			receipts.path(),
		),
	);
	assert!(
		gitfs::verification::validate_candidate(root, "advance-owner", &candidate, &revision)
			.unwrap()
			.is_some()
	);
	std::fs::write(root.join("trunk.txt"), b"advanced\n").unwrap();
	git(root, &["add", "trunk.txt"]);
	git(root, &["commit", "-qm", "advance"]);
	assert!(
		gitfs::verification::validate_candidate(root, "advance-owner", &candidate, &revision)
			.is_err()
	);
}

#[test]
fn parallel_sessions_keep_candidate_receipts_and_logs_independent() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let source_a = source(root, "concurrent-a", "a.txt");
	let source_b = source(root, "concurrent-b", "b.txt");
	let revision_a = revision_for(&trusted, "concurrent-a", &source_a);
	let revision_b = revision_for(&trusted, "concurrent-b", &source_b);
	assert_ne!(revision_a, revision_b);
	store_receipt(
		receipts.path(),
		&revision_a,
		&receipt(
			&trusted,
			"concurrent-a",
			&source_a,
			&revision_a,
			receipts.path(),
		),
	);
	store_receipt(
		receipts.path(),
		&revision_b,
		&receipt(
			&trusted,
			"concurrent-b",
			&source_b,
			&revision_b,
			receipts.path(),
		),
	);
	assert!(
		gitfs::verification::validate_candidate(root, "concurrent-a", &source_a, &revision_a)
			.unwrap()
			.is_some()
	);
	assert!(
		gitfs::verification::validate_candidate(root, "concurrent-b", &source_b, &revision_b)
			.unwrap()
			.is_some()
	);
	assert!(receipts
		.path()
		.join(format!("{revision_a}.stdout"))
		.exists());
	assert!(receipts
		.path()
		.join(format!("{revision_b}.stdout"))
		.exists());
}

#[test]
fn policy_only_push_hook_requires_matching_verified_commit_subject_author_and_receipt() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let trusted = policy(receipts.path());
	set_policy(root, &trusted);
	let session = "push-owner";
	let reviewed = source(root, session, "first.txt");
	let revision = revision_for(&trusted, session, &reviewed);
	let pass = receipt(&trusted, session, &reviewed, &revision, receipts.path());
	let commit = integrated_commit(
		root,
		&reviewed,
		session,
		&revision,
		"candidate commit",
		"verification",
	);

	// Without a configured command-review policy, verification still applies.
	assert!(gitfs::review::check_commit(root, &commit).is_err());
	store_receipt(receipts.path(), &revision, &pass);
	gitfs::review::check_commit(root, &commit).unwrap();
	gitfs::review::check_range(root, reviewed["head"].as_str().unwrap(), &commit).unwrap();

	let wrong_subject = integrated_commit(
		root,
		&reviewed,
		session,
		&revision,
		"different subject",
		"verification",
	);
	let wrong_author = integrated_commit(
		root,
		&reviewed,
		session,
		&revision,
		"candidate commit",
		"someone else",
	);
	assert!(gitfs::review::check_commit(root, &wrong_subject).is_err());
	assert!(gitfs::review::check_commit(root, &wrong_author).is_err());
	assert!(
		gitfs::review::check_range(root, reviewed["head"].as_str().unwrap(), &wrong_subject)
			.is_err()
	);
	let mut cancelled = pass;
	cancelled["checks"][0]["status"] = json!("cancelled");
	store_receipt(receipts.path(), &revision, &cancelled);
	assert!(gitfs::review::check_commit(root, &commit).is_err());
}

#[test]
fn coordinator_runs_a_real_check_inside_an_exact_candidate_snapshot() {
	let (repo, receipts) = fixture();
	let root = repo.path();
	let mut trusted = policy(receipts.path());
	trusted["checks"][0]["definition"] = json!("git -C {candidate} rev-parse HEAD^{tree}");
	set_policy(root, &trusted);
	let session = "command-owner";
	let reviewed = source(root, session, "first.txt");
	let store = StoreConfig::new(".git-fs").at(root).unwrap();
	let candidate = store
		.create_candidate_snapshot_checked(
			session,
			Some(reviewed["tree"].as_str().unwrap()),
			Some(&reviewed),
		)
		.unwrap();
	let checkout = Path::new(candidate["path"].as_str().unwrap());
	let check = Command::new("git")
		.arg("-C")
		.arg(checkout)
		.args(["rev-parse", "HEAD^{tree}"])
		.output()
		.unwrap();
	assert!(check.status.success());
	assert_eq!(
		String::from_utf8_lossy(&check.stdout).trim(),
		reviewed["tree"].as_str().unwrap()
	);
	// The runner's build output is separate from candidate source.
	let build = receipts.path().join("check-build.bin");
	std::fs::write(&build, b"fixture-build-output").unwrap();
	let revision = revision_for(&trusted, session, &reviewed);
	let mut pass = receipt(&trusted, session, &reviewed, &revision, receipts.path());
	let stdout = receipts.path().join(format!("{revision}.stdout"));
	let stderr = receipts.path().join(format!("{revision}.stderr"));
	std::fs::write(&stdout, &check.stdout).unwrap();
	std::fs::write(&stderr, &check.stderr).unwrap();
	pass["checks"][0]["stdout"]["sha256"] = json!(digest(&check.stdout));
	pass["checks"][0]["stderr"]["sha256"] = json!(digest(&check.stderr));
	pass["checks"][0]["output_digests"] = json!([{
		"path": build.to_string_lossy(), "sha256": digest(b"fixture-build-output")
	}]);
	store_receipt(receipts.path(), &revision, &pass);
	assert!(
		gitfs::verification::validate_candidate(root, session, &reviewed, &revision)
			.unwrap()
			.is_some()
	);

	// The actual runner now observes a changed input instead of recycling its pass.
	std::fs::write(checkout.join("first.txt"), b"tampered candidate\n").unwrap();
	let changed = Command::new("git")
		.arg("-C")
		.arg(checkout)
		.args(["status", "--porcelain", "--untracked-files=all"])
		.output()
		.unwrap();
	assert!(changed.status.success() && !changed.stdout.is_empty());
	let mut invalid = pass.clone();
	invalid["checks"][0]["input_integrity"] = json!("changed");
	store_receipt(receipts.path(), &revision, &invalid);
	assert!(gitfs::verification::validate_candidate(root, session, &reviewed, &revision).is_err());

	std::fs::write(checkout.join("first.txt"), b"candidate\n").unwrap();
	store
		.release_candidate_snapshot(
			session,
			candidate["id"].as_str().unwrap(),
			candidate["capability"].as_str().unwrap(),
		)
		.unwrap();
}
