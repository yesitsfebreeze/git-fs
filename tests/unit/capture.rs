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
fn selection_is_exact_empty_is_empty_and_unowned_is_rejected_before_writes() {
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
	let result = dispatch(&store, "s", &json!({"op":"capture"})).unwrap();
	assert_eq!(result["error"], true);
	let result = payload(result);
	assert_eq!(result["captured"], json!(["B"]));
	assert_eq!(result["errors"].as_array().unwrap().len(), 1);
	assert!(result["errors"][0]
		.as_str()
		.unwrap()
		.starts_with("A: read failed:"));
	assert_eq!(
		store.read("s", Path::new("A")).unwrap().unwrap(),
		b"overlay-A"
	);
	std::fs::create_dir(dir.path().join("A")).unwrap();
	let unreadable = dispatch(&store, "s", &json!({"op":"capture","paths":["A"]})).unwrap();
	assert_eq!(unreadable["error"], true);
	assert_eq!(payload(unreadable)["captured"], json!([]));
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
	let out = dispatch_captured(&store, "s", &json!({"op":"capture"}), &mut capture).unwrap();
	assert_eq!(out["error"], true);
	assert_eq!(capture.records.len(), 2);
	assert!(capture
		.records
		.iter()
		.all(|r| r.target.path.ends_with("/A")));
}
