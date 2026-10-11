use super::*;
#[test]
fn context_is_bounded_and_missing_coordinates_stay_null() {
	let c = Capture::from_context(&json!({"session":"s","cwd":"/tmp"})).unwrap();
	assert!(c.actor.unwrap().run.is_none());
	for v in [
		json!({"session":"s","run":true}),
		json!({"session":"s","call":"x".repeat(257)}),
		json!({"session":""}),
	] {
		assert!(Capture::from_context(&v).is_err());
	}
}

fn fixture() -> (tempfile::TempDir, Store) {
	let dir = tempfile::tempdir().unwrap();
	let store = crate::store::StoreConfig::new(".kern/gitfs");
	let git = |args: &[&str]| {
		let out = std::process::Command::new("git")
			.arg("-C")
			.arg(dir.path())
			.args(args)
			.env("GIT_AUTHOR_NAME", "fixture")
			.env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
			.env("GIT_COMMITTER_NAME", "fixture")
			.env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
			.output()
			.unwrap();
		assert!(
			out.status.success(),
			"{}",
			String::from_utf8_lossy(&out.stderr)
		);
	};
	git(&["init", "-q"]);
	std::fs::write(dir.path().join("base"), "base").unwrap();
	git(&["add", "base"]);
	git(&["commit", "-qm", "base"]);
	let s = store.at(dir.path()).unwrap();
	(dir, s)
}
fn capture() -> Capture {
	Capture::from_context(&json!({"session":"s","run":"r","call":"c"})).unwrap()
}
#[test]
fn locked_overlay_receipts_pin_old_blob_tip_and_first_parent_without_inventing_overlay() {
	let (_dir, store) = fixture();
	let head = store.head().unwrap().unwrap();
	let mut c = capture();
	let first = store
		.write_blob_captured(
			"s",
			Path::new("base"),
			b"first",
			"100644",
			&mut c,
			Operation::Write,
		)
		.unwrap()
		.unwrap();
	let e = &c.records[0];
	assert_eq!(e.overlay_commit.before, None);
	assert_eq!(e.overlay_commit.after, Some(first.clone()));
	assert_eq!(
		e.before,
		Version::GitObject {
			value: store
				.git_str(&["rev-parse", &format!("{head}:base")])
				.unwrap()
				.trim()
				.into()
		}
	);
	assert_eq!(
		store
			.git_str(&["rev-parse", &format!("{first}^")])
			.unwrap()
			.trim(),
		head
	);
	assert!(store
		.write_blob_captured(
			"s",
			Path::new("base"),
			b"first",
			"100644",
			&mut c,
			Operation::Write
		)
		.unwrap()
		.is_none());
	assert_eq!(c.records.len(), 1);
	let second = store
		.write_blob_captured(
			"s",
			Path::new("base"),
			b"second",
			"100644",
			&mut c,
			Operation::Edit,
		)
		.unwrap()
		.unwrap();
	assert_eq!(c.records[1].before, c.records[0].after);
	assert_eq!(c.records[1].overlay_commit.before, Some(first));
	assert_eq!(c.records[1].overlay_commit.after, Some(second));
	assert_ne!(c.records[0].publication_id, c.records[1].publication_id);
	assert!(store
		.write_blob_captured(
			"other",
			Path::new("x"),
			b"x",
			"100644",
			&mut c,
			Operation::Write
		)
		.is_err());
}
#[test]
fn concurrent_same_session_receipts_form_the_actual_locked_commit_chain() {
	let (_dir, store) = fixture();
	let collected = std::sync::Mutex::new(Vec::new());
	std::thread::scope(|scope| {
		for n in 0..8 {
			let store = &store;
			let collected = &collected;
			scope.spawn(move || {
				let mut c = capture();
				store
					.write_blob_captured(
						"s",
						Path::new("a"),
						n.to_string().as_bytes(),
						"100644",
						&mut c,
						Operation::Write,
					)
					.unwrap();
				collected.lock().unwrap().extend(c.records);
			});
		}
	});
	let rows = collected.into_inner().unwrap();
	assert_eq!(rows.len(), 8);
	let mut prior = None;
	for _ in 0..8 {
		let row = rows
			.iter()
			.find(|r| r.overlay_commit.before == prior)
			.unwrap();
		if let Some(ref tip) = prior {
			assert_eq!(
				store
					.git_str(&[
						"rev-parse",
						&format!("{}^", row.overlay_commit.after.as_ref().unwrap())
					])
					.unwrap()
					.trim(),
				tip
			);
		}
		prior = row.overlay_commit.after.clone();
	}
	assert_eq!(prior, store.tip("s").unwrap());
}
#[test]
fn materialize_keeps_exact_applied_evidence_and_reports_failed_deletion_and_ledger() {
	let (dir, store) = fixture();
	store
		.write_blob("s", Path::new("a"), b"overlay", "100644")
		.unwrap();
	std::fs::write(dir.path().join("a"), "disk").unwrap();
	let mut c = capture();
	let out = store
		.materialize_captured("s", Some(&["a".into()]), true, &mut c)
		.unwrap();
	assert_eq!(out.applied, vec!["a"]);
	assert!(out.errors.is_empty());
	assert_eq!(c.records[0].before, content_version(b"disk"));
	assert_eq!(c.records[0].after, content_version(b"overlay"));
	assert_eq!(c.records[0].target.storage, Storage::Direct);
	// Make a tracked deletion in the overlay via ordinary plumbing; directory unlink must fail.
	let tip = store.tip("s").unwrap().unwrap();
	let tree = store
		.git_str(&["rev-parse", &format!("{tip}^{{tree}}")])
		.unwrap();
	let index = dir.path().join("delete-index");
	let git = |args: &[&str]| {
		let out = std::process::Command::new("git")
			.arg("-C")
			.arg(store.root())
			.args(args)
			.env("GIT_INDEX_FILE", &index)
			.output()
			.unwrap();
		assert!(out.status.success());
		String::from_utf8(out.stdout).unwrap().trim().to_owned()
	};
	git(&["read-tree", tree.trim()]);
	git(&["update-index", "--force-remove", "base"]);
	let tree = git(&["write-tree"]);
	let commit = store
		.git_str(&["commit-tree", &tree, "-p", &tip, "-m", "deletion"])
		.unwrap();
	store
		.git(&["update-ref", &store.branch("s"), commit.trim()])
		.unwrap();
	std::fs::remove_file(dir.path().join("base")).unwrap();
	std::fs::create_dir(dir.path().join("base")).unwrap();
	std::fs::write(dir.path().join("base/keep"), "keep").unwrap();
	let mut c = capture();
	let out = store
		.materialize_captured("s", Some(&["base".into()]), true, &mut c)
		.unwrap();
	assert!(out.applied.is_empty());
	assert!(!out.errors.is_empty());
	assert!(c.records.is_empty());
	assert!(dir.path().join("base/keep").exists());
	std::fs::remove_dir_all(dir.path().join("base")).unwrap();
	std::fs::write(dir.path().join("base"), "base").unwrap();
	let out = store
		.materialize_captured("s", Some(&["base".into()]), true, &mut c)
		.unwrap();
	assert_eq!(out.applied, vec!["base"]);
	assert_eq!(c.records[0].after, Version::Absent {});
	assert!(!dir.path().join("base").exists());
	let ledger = dir.path().join(".kern/gitfs/s.materialized.json");
	std::fs::remove_file(&ledger).unwrap();
	std::fs::create_dir(&ledger).unwrap();
	let mut c = capture();
	let out = store
		.materialize_captured("s", Some(&["a".into()]), true, &mut c)
		.unwrap();
	assert_eq!(out.applied, vec!["a"]);
	assert!(out.errors.iter().any(|e| e.contains("ledger")));
	assert_eq!(c.records.len(), 1);
}
#[test]
fn metadata_caps_and_unreadable_preimage_never_claim_complete_attribution() {
	let (dir, store) = fixture();
	let mut c = capture();
	let mut sample = c
		.reserve(
			&store,
			Path::new("a"),
			Operation::Materialize,
			Storage::Direct,
		)
		.unwrap()
		.unwrap();
	sample.overlay_commit.after = Some("a".repeat(40));
	for _ in 0..record::batch() + 6 {
		let mut e = sample.clone();
		e.publication_id = record::publication_id().unwrap();
		c.keep(Some(e));
	}
	assert_eq!(c.records.len(), record::batch());
	assert_eq!(c.omitted, 6);
	let mut c = capture();
	// Few large records reach the byte cap: `keep` reserializes the whole batch
	// on every call, so many small ones make this quadratic.
	let mut large = sample.clone();
	large.target.workspace = format!("/{}", "a".repeat(1_000_000));
	large.target.path = format!("{}/b", large.target.workspace);
	for _ in 0..10 {
		let mut e = large.clone();
		e.publication_id = record::publication_id().unwrap();
		c.keep(Some(e));
	}
	assert!(c.records.len() < record::batch());
	assert!(c.omitted > 0);
	assert!(serde_json::to_vec(&c.args()).unwrap().len() <= record::batch_bytes());
	store
		.write_blob("s", Path::new("a"), b"overlay", "100644")
		.unwrap();
	// An unreadable preimage: not a regular file, so it is refused before it is
	// opened. The oversized case takes the same branch; this one does not need
	// a `limits.provenance_bytes`-sized write to reach it.
	std::os::unix::fs::symlink("elsewhere", dir.path().join("a")).unwrap();
	let mut c = capture();
	let out = store.materialize_captured("s", None, true, &mut c).unwrap();
	assert_eq!(out.applied, vec!["a"]);
	assert!(c.records.is_empty());
	assert_eq!(c.unknown, 1);
	assert_eq!(std::fs::read(dir.path().join("a")).unwrap(), b"overlay");
}
#[test]
fn nonregular_or_alias_preimages_are_rejected_without_waiting_for_a_writer() {
	let (dir, _store) = fixture();
	let fifo = dir.path().join("fifo");
	assert!(std::process::Command::new("mkfifo")
		.arg(&fifo)
		.status()
		.unwrap()
		.success());
	let start = std::time::Instant::now();
	assert!(preimage(&fifo).is_err());
	assert!(start.elapsed() < std::time::Duration::from_secs(1));
	let alias = dir.path().join("alias");
	std::os::unix::fs::symlink(dir.path().join("base"), &alias).unwrap();
	assert!(preimage(&alias).is_err());
}

#[test]
fn real_bulk_materialization_preserves_effects_after_record_cap_and_late_write_failure() {
	let (dir, store) = fixture();
	let names = (0..record::batch() + 2)
		.map(|n| format!("bulk{n:04}"))
		.collect::<Vec<_>>();
	for name in &names {
		std::fs::write(dir.path().join(name), name).unwrap();
	}
	let mut args = vec!["add", "--"];
	args.extend(names.iter().map(String::as_str));
	store.git(&args).unwrap();
	let tree = store.git_str(&["write-tree"]).unwrap();
	let head = store.head().unwrap().unwrap();
	let commit = store
		.git_str(&[
			"commit-tree",
			tree.trim(),
			"-p",
			&head,
			"-m",
			"bulk fixture",
		])
		.unwrap();
	store
		.git(&["update-ref", &store.branch("s"), commit.trim()])
		.unwrap();
	for name in &names {
		std::fs::remove_file(dir.path().join(name)).unwrap();
	}
	let last = names.last().unwrap().clone();
	std::fs::create_dir(dir.path().join(&last)).unwrap();
	let mut c = capture();
	let out = store.materialize_captured("s", None, true, &mut c).unwrap();
	assert_eq!(out.applied.len(), record::batch() + 1);
	assert_eq!(out.errors.len(), 1);
	assert_eq!(c.records.len(), record::batch());
	assert_eq!(c.omitted, 1);
	assert_eq!(c.unknown, 1);
	assert!(serde_json::to_vec(&c.args()).unwrap().len() <= record::batch_bytes());
	for name in names.iter().take(record::batch() + 1) {
		assert_eq!(
			std::fs::read_to_string(dir.path().join(name)).unwrap(),
			*name
		);
	}
	assert!(dir.path().join(&last).is_dir());
}
