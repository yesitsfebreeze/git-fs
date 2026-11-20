use super::*;
use crate::store::Store;

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

fn fixture() -> (tempfile::TempDir, Store) {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q"]);
	git(
		dir.path(),
		&[
			"-c",
			"commit.gpgsign=false",
			"commit",
			"--allow-empty",
			"-qm",
			"fixture",
		],
	);
	let store = StoreConfig::new(".kern/gitfs").at(dir.path()).unwrap();
	for path in ["A", "B"] {
		store
			.write_blob(
				"s",
				Path::new(path),
				format!("overlay-{path}").as_bytes(),
				"100755",
			)
			.unwrap();
		std::fs::write(dir.path().join(path), format!("disk-{path}")).unwrap();
	}
	(dir, store)
}

fn payload(value: Value) -> Value {
	value["content"].clone()
}

#[test]
fn capture_adopts_explicit_local_edits_additions_and_deletions_without_shared_staging() {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q"]);
	for path in ["edit", "delete", "unrelated"] {
		std::fs::write(dir.path().join(path), "before\n").unwrap();
	}
	git(dir.path(), &["add", "."]);
	git(
		dir.path(),
		&["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
	);
	let store = StoreConfig::new(".kern/gitfs").at(dir.path()).unwrap();
	std::fs::write(dir.path().join("unrelated"), "staged\n").unwrap();
	git(dir.path(), &["add", "unrelated"]);
	std::fs::write(dir.path().join("unrelated"), "unstaged\n").unwrap();
	let index = std::fs::read(dir.path().join(".git/index")).unwrap();
	let head = git(dir.path(), &["rev-parse", "HEAD"]);
	std::fs::write(dir.path().join("edit"), "after\n").unwrap();
	std::fs::write(dir.path().join("add"), "new\n").unwrap();
	std::fs::remove_file(dir.path().join("delete")).unwrap();
	assert_eq!(
		payload(dispatch(&store, "s", &json!({"op":"capture"})).unwrap())["captured"],
		json!([])
	);
	for paths in [json!(["edit", "missing"]), json!(["edit", "../escape"])] {
		assert!(dispatch(&store, "s", &json!({"op":"capture","paths":paths})).is_err());
		assert!(store.tip("s").unwrap().is_none());
	}
	let out = dispatch(
		&store,
		"s",
		&json!({"op":"capture","paths":["edit","add","delete","edit"]}),
	)
	.unwrap();
	assert_eq!(out["error"], false, "{out}");
	assert_eq!(payload(out)["captured"], json!(["edit", "add", "delete"]));
	assert_eq!(
		git(
			dir.path(),
			&["diff", "--name-status", "HEAD", "refs/gitfs/s"]
		),
		"A\tadd\nD\tdelete\nM\tedit\n"
	);
	assert_eq!(
		store.read("s", Path::new("edit")).unwrap().unwrap(),
		b"after\n"
	);
	assert!(store.read("s", Path::new("delete")).unwrap().is_none());
	assert_eq!(std::fs::read(dir.path().join(".git/index")).unwrap(), index);
	assert_eq!(git(dir.path(), &["rev-parse", "HEAD"]), head);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("unrelated")).unwrap(),
		"unstaged\n"
	);
}

#[test]
fn capture_records_owned_deletions_and_explicit_recreation() {
	let (dir, store) = fixture();
	std::fs::remove_file(dir.path().join("A")).unwrap();
	let out = dispatch(&store, "s", &json!({"op":"capture","paths":["A"]})).unwrap();
	assert_eq!(out["error"], false, "{out}");
	assert_eq!(payload(out)["captured"], json!(["A"]));
	assert!(store.read("s", Path::new("A")).unwrap().is_none());
	let tip = store.tip("s").unwrap();
	dispatch(&store, "s", &json!({"op":"capture","paths":["A"]})).unwrap();
	assert_eq!(store.tip("s").unwrap(), tip);
	std::fs::write(dir.path().join("A"), "restored\n").unwrap();
	let out = dispatch(&store, "s", &json!({"op":"capture","paths":["A"]})).unwrap();
	assert_eq!(out["error"], false, "{out}");
	assert_eq!(
		store.read("s", Path::new("A")).unwrap().unwrap(),
		b"restored\n"
	);
}

#[test]
fn selection_is_exact_empty_is_empty_and_unknown_missing_is_rejected_before_writes() {
	let (dir, store) = fixture();
	std::fs::write(dir.path().join("unrelated"), "staged").unwrap();
	git(dir.path(), &["add", "unrelated"]);
	std::fs::write(dir.path().join("unrelated"), "unstaged").unwrap();
	git(dir.path(), &["update-ref", "refs/fixture/keep", "HEAD"]);
	let index = std::fs::read(dir.path().join(".git/index")).unwrap();
	let refs = git(
		dir.path(),
		&[
			"for-each-ref",
			"--format=%(refname) %(objectname)",
			"refs/heads/",
			"refs/fixture/",
		],
	);
	let tip = store.tip("s").unwrap();
	assert_eq!(
		payload(dispatch(&store, "s", &json!({"op":"capture","paths":[]})).unwrap())["captured"],
		json!([])
	);
	assert_eq!(store.tip("s").unwrap(), tip);
	for paths in [json!(["A", "unowned"]), json!(["A", "../escape"])] {
		assert!(dispatch(&store, "s", &json!({"op":"capture","paths":paths})).is_err());
		assert_eq!(store.tip("s").unwrap(), tip);
	}
	let result = dispatch(&store, "s", &json!({"op":"capture","paths":["A","A"]})).unwrap();
	assert_eq!(result["error"], false);
	assert_eq!(payload(result)["captured"], json!(["A"]));
	assert_eq!(store.read("s", Path::new("A")).unwrap().unwrap(), b"disk-A");
	assert_eq!(
		store.read("s", Path::new("B")).unwrap().unwrap(),
		b"overlay-B"
	);
	assert_eq!(
		store.blob_for("s", Path::new("A")).unwrap().unwrap().1,
		"100755"
	);
	assert_eq!(std::fs::read(dir.path().join(".git/index")).unwrap(), index);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("unrelated")).unwrap(),
		"unstaged"
	);
	assert_eq!(
		git(
			dir.path(),
			&[
				"for-each-ref",
				"--format=%(refname) %(objectname)",
				"refs/heads/",
				"refs/fixture/"
			]
		),
		refs
	);
	let result = dispatch(&store, "s", &json!({"op":"capture"})).unwrap();
	assert_eq!(payload(result)["captured"], json!(["B"]));
}

#[test]
fn read_failures_report_partial_success_and_preserve_failed_overlay() {
	let (dir, store) = fixture();
	std::fs::remove_file(dir.path().join("A")).unwrap();
	std::fs::create_dir(dir.path().join("A")).unwrap();
	let result = dispatch(&store, "s", &json!({"op":"capture"})).unwrap();
	assert_eq!(result["error"], true);
	let result = payload(result);
	assert_eq!(result["captured"], json!(["B"]));
	assert_eq!(result["errors"].as_array().unwrap().len(), 1);
	assert!(result["errors"][0].as_str().unwrap().starts_with("A: "));
	assert_eq!(
		store.read("s", Path::new("A")).unwrap().unwrap(),
		b"overlay-A"
	);
	assert!(dispatch(&store, "s", &json!({"op":"capture","paths":["A"]})).is_err());
}

#[test]
fn bulk_capture_commits_a_move_once_and_is_idempotent() {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q"]);
	std::fs::create_dir(dir.path().join("old")).unwrap();
	for n in 0..40 {
		std::fs::write(dir.path().join("old").join(format!("f{n}")), "before\n").unwrap();
	}
	git(dir.path(), &["add", "."]);
	git(
		dir.path(),
		&["-c", "commit.gpgsign=false", "commit", "-qm", "base"],
	);
	let store = StoreConfig::new(".kern/gitfs").at(dir.path()).unwrap();
	let head = git(dir.path(), &["rev-parse", "HEAD"]).trim().to_owned();
	std::fs::create_dir(dir.path().join("new")).unwrap();
	let mut paths = Vec::new();
	for n in 0..40 {
		std::fs::remove_file(dir.path().join("old").join(format!("f{n}"))).unwrap();
		std::fs::write(dir.path().join("new").join(format!("f{n}")), "after\n").unwrap();
		paths.push(format!("old/f{n}"));
		paths.push(format!("new/f{n}"));
	}
	let out = dispatch(&store, "s", &json!({"op":"capture","paths":paths})).unwrap();
	assert_eq!(out["error"], false, "{out}");
	assert_eq!(payload(out)["captured"].as_array().unwrap().len(), 80);
	assert_eq!(
		git(
			dir.path(),
			&["rev-list", "--count", &format!("{head}..refs/gitfs/s")]
		),
		"1\n"
	);
	assert_eq!(
		git(
			dir.path(),
			&[
				"rev-list",
				"--count",
				&format!("{head}..refs/gitfs-live/current")
			]
		),
		"1\n"
	);
	assert_eq!(
		store.read("s", Path::new("new/f0")).unwrap().unwrap(),
		b"after\n"
	);
	assert!(store.read("s", Path::new("old/f0")).unwrap().is_none());
	let tip = store.tip("s").unwrap();
	let out = dispatch(
		&store,
		"s",
		&json!({"op":"capture","paths":["new/f0","old/f0"]}),
	)
	.unwrap();
	assert_eq!(payload(out)["captured"], json!([]));
	assert_eq!(store.tip("s").unwrap(), tip);
}

#[test]
fn bulk_capture_reports_per_path_errors_beside_adopted_paths() {
	let (dir, store) = fixture();
	std::fs::remove_file(dir.path().join("A")).unwrap();
	std::fs::remove_file(dir.path().join("B")).unwrap();
	std::fs::create_dir(dir.path().join("B")).unwrap();
	let out = dispatch(&store, "s", &json!({"op":"capture"})).unwrap();
	assert_eq!(out["error"], true);
	let out = payload(out);
	assert_eq!(out["captured"], json!(["A"]));
	assert_eq!(out["errors"].as_array().unwrap().len(), 1);
	assert!(out["errors"][0].as_str().unwrap().starts_with("B: "));
	assert!(store.read("s", Path::new("A")).unwrap().is_none());
	assert_eq!(
		store.read("s", Path::new("B")).unwrap().unwrap(),
		b"overlay-B"
	);
}

#[test]
fn selected_capture_records_only_new_commits_and_retains_partial_evidence() {
	let (dir, store) = fixture();
	let mut capture = crate::provenance::Capture::from_context(&json!({"session":"s"})).unwrap();
	let out = dispatch_captured(
		&store,
		"s",
		&json!({"op":"capture","paths":["A"]}),
		&mut capture,
	)
	.unwrap();
	assert_eq!(payload(out)["captured"], json!(["A"]));
	assert_eq!(capture.records.len(), 1);
	assert_eq!(
		capture.records[0].operation,
		crate::change_record::Operation::Capture
	);
	assert!(capture.records[0].actor.run.is_none());
	let before = capture.records.len();
	dispatch_captured(
		&store,
		"s",
		&json!({"op":"capture","paths":["A"]}),
		&mut capture,
	)
	.unwrap();
	assert_eq!(capture.records.len(), before);
	std::fs::write(dir.path().join("A"), "new-again").unwrap();
	std::fs::remove_file(dir.path().join("B")).unwrap();
	std::fs::create_dir(dir.path().join("B")).unwrap();
	let out = dispatch_captured(&store, "s", &json!({"op":"capture"}), &mut capture).unwrap();
	assert_eq!(out["error"], true);
	assert_eq!(capture.records.len(), 2);
	assert!(capture.records.iter().all(|r| Path::new(&r.target.path)
		.file_name()
		.is_some_and(|n| n == "A")));
}
