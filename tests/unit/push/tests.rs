use super::*;
#[test]
fn hook_requires_exactly_one_complete_advertised_update() {
	assert_eq!(git_path("/repo \n").unwrap(), PathBuf::from("/repo "));
	assert_eq!(git_path("/repo\n\n").unwrap(), PathBuf::from("/repo\n"));
	let old = "a".repeat(40);
	let new = "b".repeat(40);
	let reference = "refs/heads/main";
	let line = format!("{new} {new} {reference} {old}\n");
	assert!(hook_valid(line.as_bytes(), &old, &new, reference));
	for bad in [
		String::new(),
		format!("{line}{line}"),
		line.replace(&old, &"c".repeat(40)),
		line.replace(reference, "refs/heads/other"),
		line.replace(&new, &"d".repeat(40)),
		"x".repeat(8193),
	] {
		assert!(!hook_valid(bad.as_bytes(), &old, &new, reference));
	}
}
#[test]
fn command_timeout_and_overflow_are_bounded_and_do_not_expose_stderr() {
	let dir = tempfile::tempdir().unwrap();
	let cancel = AtomicBool::new(false);
	let cx = Context {
		root: dir.path().into(),
		dir: dir.path().join("state"),
		deadline: Instant::now() + Duration::from_millis(150),
		cancel: &cancel,
	};
	let mut cmd = Command::new("/bin/sh");
	cmd.args(["-c", "echo SECRET >&2; sleep 30"]);
	let before = Instant::now();
	let err = cx.run(cmd).err().unwrap();
	assert!(before.elapsed() < Duration::from_secs(2));
	assert!(!err.contains("SECRET"));
	let cx = Context {
		deadline: Instant::now() + Duration::from_secs(3),
		..cx
	};
	let mut cmd = Command::new("/bin/sh");
	cmd.args(["-c", "yes SECRET"]);
	assert!(cx.run(cmd).err().unwrap().contains("limit"));
}

fn sample_record(id: &str) -> Record {
	let binding = Binding {
		repository: "a".repeat(64),
		session: "s".into(),
		remote: "origin".into(),
		reference: "refs/heads/main".into(),
		commit: "b".repeat(40),
		expected_remote_head: Some("c".repeat(40)),
		url_digest: "d".repeat(64),
		timeout_ms: 1000,
	};
	Record {
		schema: "gitfs-push/v1".into(),
		operation_id: id.into(),
		revision: revision(&binding),
		binding,
		status: Status::Unknown,
		reason: "dispatch_pending_or_unknown".into(),
	}
}
#[test]
fn durable_attempt_capacity_corruption_and_monotonic_readback() {
	let temp = tempfile::tempdir().unwrap();
	let cancel = AtomicBool::new(false);
	let cx = Context {
		root: temp.path().into(),
		dir: temp.path().join("attempts"),
		// Long enough that building a `limits.push_records`-sized store cannot
		// expire it: every `lock` re-checks the deadline, and a lock that fails
		// makes `persist_readback` return the record it read instead of
		// saving one, which is not what this proof is about.
		deadline: Instant::now() + Duration::from_secs(600),
		cancel: &cancel,
	};
	{
		let _lock = cx.lock().unwrap();
	}
	for i in 0..capacity() {
		let record = sample_record(&format!("op{i}"));
		let dir = cx.dir.join(record_key("s", &record.operation_id));
		private_dir(&dir).unwrap();
		cx.save(&dir, &record).unwrap();
	}
	assert_eq!(cx.records().unwrap().len(), capacity());
	let original = sample_record("op0");
	let dir = cx.dir.join(record_key("s", "op0"));
	let mut confirmed = original.clone();
	confirmed.status = Status::Confirmed;
	confirmed.reason = "exact_remote_confirmed".into();
	cx.persist_readback(&dir, confirmed).unwrap();
	assert_eq!(
		cx.persist_readback(&dir, original).unwrap().status,
		Status::Confirmed
	);
	// Simulate a crash after the hook durably refused but before record update.
	let unknown = sample_record("op2");
	let interrupted = cx.dir.join(record_key("s", "op2"));
	atomic_write(&interrupted.join("hook-result"), b"refused").unwrap();
	assert_eq!(
		cx.reconcile(&interrupted, unknown).unwrap().status,
		Status::Refused
	);
	let extra = sample_record("extra");
	let dir = cx.dir.join(record_key("s", "extra"));
	private_dir(&dir).unwrap();
	cx.save(&dir, &extra).unwrap();
	assert!(cx.records().err().unwrap().contains("capacity"));
	fs::remove_dir_all(dir).unwrap();
	let dir = cx.dir.join(record_key("s", "op1"));
	fs::write(dir.join("record.json"), b"{}").unwrap();
	assert!(cx.records().is_err());
}
#[test]
fn cancelled_command_kills_the_spawned_helper_group() {
	let temp = tempfile::tempdir().unwrap();
	let pidfile = temp.path().join("pid");
	let cancel = Arc::new(AtomicBool::new(false));
	let flag = cancel.clone();
	let root = temp.path().to_owned();
	let path = pidfile.clone();
	let worker = std::thread::spawn(move || {
		let cx = Context {
			root: root.clone(),
			dir: root,
			deadline: Instant::now() + Duration::from_secs(4),
			cancel: &flag,
		};
		let mut command = Command::new("/bin/sh");
		command
			.args([
				"-c",
				"sh -c 'echo $$ > \"$1\"; exec sleep 30' helper \"$1\" & wait",
				"parent",
			])
			.arg(path);
		cx.run(command).err().unwrap()
	});
	let until = Instant::now() + Duration::from_secs(2);
	while !pidfile.exists() && Instant::now() < until {
		std::thread::sleep(Duration::from_millis(5));
	}
	let pid = fs::read_to_string(&pidfile).unwrap();
	cancel.store(true, Ordering::SeqCst);
	assert!(worker.join().unwrap().contains("cancelled"));
	let until = Instant::now() + Duration::from_secs(2);
	loop {
		let exists = Command::new("/bin/kill")
			.args(["-0", pid.trim()])
			.stderr(Stdio::null())
			.status()
			.unwrap()
			.success();
		if !exists {
			break;
		}
		assert!(Instant::now() < until, "helper survived group cancellation");
		std::thread::sleep(Duration::from_millis(5));
	}
}

#[test]
fn receipt_looking_prose_or_conflicting_trailers_do_not_authorize_push() {
	let block = format!(
		"Integrated-by: gitfs\nGitfs-Session: s\nGitfs-Review: {}\nGitfs-Consumed: {}",
		"a".repeat(64),
		"c".repeat(40)
	);
	assert!(reviewed_trailers(&format!("subject\n\n{block}\n\n"), "s"));
	for body in [
		format!("subject\n\n{block}\nordinary prose\n"),
		format!("subject\n\n{block}\n\nordinary prose\n"),
		format!("subject\n\n{block}\nGitfs-Session: other\n"),
		format!("subject\n\n{block}\nGitfs-Review: {}\n", "b".repeat(64)),
		format!("subject\n{block}\n"),
	] {
		assert!(!reviewed_trailers(&body, "s"), "{body}");
	}
	assert!(!reviewed_trailers(&format!("subject\n\n{block}"), "other"));
}

#[test]
fn the_block_integrate_writes_is_the_block_push_accepts() {
	let block = crate::integrate::receipt("s", &"a".repeat(64), &"b".repeat(40));
	assert!(
		reviewed_trailers(&format!("subject\n\n{block}"), "s"),
		"{block}"
	);
}

#[test]
fn a_commit_without_operation_id_says_what_to_pick() {
	let dir = tempfile::tempdir().unwrap();
	assert!(Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	let cancel = AtomicBool::new(false);
	let error = execute(
		&crate::store::StoreConfig::new(".kern/gitfs"),
		dir.path(),
		"s",
		&serde_json::json!({"phase": "commit"}),
		1000,
		Instant::now() + Duration::from_secs(5),
		&cancel,
	)
	.unwrap_err();
	assert!(error.contains("operation_id: a name you pick"), "{error}");
}

#[test]
fn a_preview_carrying_commit_fields_names_phase_commit() {
	let dir = tempfile::tempdir().unwrap();
	assert!(Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	let cancel = AtomicBool::new(false);
	let error = execute(
		&crate::store::StoreConfig::new(".kern/gitfs"),
		dir.path(),
		"s",
		&serde_json::json!({"operation_id": "p1", "expected_revision": "x"}),
		1000,
		Instant::now() + Duration::from_secs(5),
		&cancel,
	)
	.unwrap_err();
	assert!(error.contains("phase: \"commit\""), "{error}");
}

fn git_in(dir: &Path, args: &[&str]) -> String {
	let out = Command::new("git")
		.args([
			"-c",
			"user.name=t",
			"-c",
			"user.email=t@t",
			"-c",
			"commit.gpgsign=false",
		])
		.args(args)
		.current_dir(dir)
		.output()
		.unwrap();
	assert!(
		out.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&out.stderr)
	);
	String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn a_bare_preview_after_a_integrate_derives_remote_ref_commit_and_remote_head() {
	let temp = tempfile::tempdir().unwrap();
	let (bare, work) = (temp.path().join("bare.git"), temp.path().join("work"));
	git_in(
		temp.path(),
		&["init", "-q", "--bare", "-b", "main", bare.to_str().unwrap()],
	);
	git_in(
		temp.path(),
		&["init", "-q", "-b", "main", work.to_str().unwrap()],
	);
	git_in(&work, &["commit", "-q", "--allow-empty", "-m", "base"]);
	git_in(&work, &["remote", "add", "origin", bare.to_str().unwrap()]);
	git_in(&work, &["push", "-q", "-u", "origin", "main"]);
	let head = git_in(&work, &["rev-parse", "HEAD"]);
	let block = crate::integrate::receipt("s", &"a".repeat(64), &"b".repeat(40));
	git_in(
		&work,
		&[
			"commit",
			"-q",
			"--allow-empty",
			"-m",
			&format!("integrated\n\n{block}"),
		],
	);
	let integrated = git_in(&work, &["rev-parse", "HEAD"]);
	git_in(
		&work,
		&["commit", "-q", "--allow-empty", "-m", "someone else"],
	);
	let cancel = AtomicBool::new(false);
	let preview = |session: &str, input: Value| {
		execute(
			&crate::store::StoreConfig::new(".kern/gitfs"),
			&work,
			session,
			&input,
			5000,
			Instant::now() + Duration::from_secs(10),
			&cancel,
		)
	};
	let answer = preview("s", json!({})).unwrap();
	let b = &answer["binding"];
	assert_eq!(b["remote"], "origin", "{answer}");
	assert_eq!(b["reference"], "refs/heads/main", "{answer}");
	assert_eq!(b["commit"], json!(integrated), "{answer}");
	assert_eq!(b["expected_remote_head"], json!(head), "{answer}");
	// explicit wins, and is still validated
	let named = preview(
		"s",
		json!({"ref": "refs/heads/next", "expected_remote_head": null}),
	)
	.unwrap();
	assert_eq!(named["binding"]["reference"], "refs/heads/next", "{named}");
	assert!(preview("s", json!({"remote": "-x"})).is_err());
	let none = preview("other", json!({})).unwrap_err();
	assert!(none.contains("integrate first"), "{none}");
}
