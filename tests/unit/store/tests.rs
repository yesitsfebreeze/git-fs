use super::*;

/// Fresh git repo with an initial commit, plus a Store over it.
fn new_repo() -> (tempfile::TempDir, Store) {
	let dir = tempfile::tempdir().unwrap();
	let git = |args: &[&str]| {
		let ok = std::process::Command::new("git")
			.arg("-C")
			.arg(dir.path())
			.args(args)
			.env("GIT_AUTHOR_NAME", "t")
			.env("GIT_AUTHOR_EMAIL", "t@t")
			.env("GIT_COMMITTER_NAME", "t")
			.env("GIT_COMMITTER_EMAIL", "t@t")
			.status()
			.unwrap()
			.success();
		assert!(ok, "git {args:?} failed");
	};
	git(&["init", "-q"]);
	std::fs::write(dir.path().join("README"), "base\n").unwrap();
	git(&["add", "-A"]);
	git(&["commit", "-q", "-m", "init"]);
	let store = StoreConfig::new(".kern/gitfs").at(dir.path()).unwrap();
	(dir, store)
}

#[test]
fn a_submodule_refusal_names_the_gitlink_the_path_is_under_only() {
	let (_dir, store) = new_repo();
	let head = store.git_str(&["rev-parse", "HEAD"]).unwrap();
	let head = head.trim();
	// Two sibling gitlinks and a plain file beside them, as in .kern/kerns.
	for sub in ["d/a", "d/b"] {
		store
			.git_str(&[
				"update-index",
				"--add",
				"--cacheinfo",
				&format!("160000,{head},{sub}"),
			])
			.unwrap();
	}
	std::fs::create_dir_all(store.root().join("d")).unwrap();
	std::fs::write(store.root().join("d/x.txt"), "x\n").unwrap();
	store.git_str(&["add", "d/x.txt"]).unwrap();
	store
		.git_str(&[
			"-c",
			"user.name=t",
			"-c",
			"user.email=t@t",
			"commit",
			"-qm",
			"gitlinks",
		])
		.unwrap();
	let err = store
		.write_blob("sb", Path::new("d/b/y.txt"), b"y\n", "100644")
		.unwrap_err();
	assert!(err.contains("inside the git submodule `d/b`"), "{err}");
	// A file beside the gitlinks is this repository's own.
	store
		.write_blob("sb", Path::new("d/x.txt"), b"x2\n", "100644")
		.unwrap();
}

#[test]
fn materialize_of_a_session_with_no_commit_answers_nothing_to_copy() {
	let (_dir, store) = new_repo();
	let r = store.materialize("fresh", None, false).unwrap();
	assert!(
		r.applied.is_empty() && r.conflicts.is_empty(),
		"{:?}",
		r.conflicts
	);
	assert!(r.errors.is_empty(), "{:?}", r.errors);
}

#[test]
fn roundtrip_and_noop() {
	let (_dir, store) = new_repo();
	let session = "s1";
	let first = store
		.write_blob(session, Path::new("a.txt"), b"x\n", "100644")
		.unwrap()
		.unwrap();
	// Same content again: no new commit.
	assert!(store
		.write_blob(session, Path::new("a.txt"), b"x\n", "100644")
		.unwrap()
		.is_none());
	// Read-your-writes works; the worktree is untouched.
	assert_eq!(
		store.read(session, Path::new("a.txt")).unwrap().unwrap(),
		b"x\n"
	);
	assert!(!store.root().join("a.txt").exists());
	// The session owns exactly that path, new.
	assert_eq!(
		store.owned_paths(session).unwrap(),
		vec![('A', "a.txt".to_owned())]
	);
	assert_ne!(first, store.head().unwrap().unwrap());
}

#[test]
fn a_nested_path_roundtrip_uses_forward_slashes_in_git() {
	let (_dir, store) = new_repo();
	let session = "nested";
	let rel = Path::new("src/geom.odin");
	store
		.write_blob(session, rel, b"package geom\n", "100644")
		.unwrap()
		.unwrap();
	assert_eq!(
		store.owned_paths(session).unwrap(),
		vec![('A', "src/geom.odin".to_owned())]
	);
	let r = store.materialize(session, None, false).unwrap();
	assert!(r.conflicts.is_empty());
	assert_eq!(r.applied, vec!["src/geom.odin".to_owned()]);
	assert_eq!(
		std::fs::read_to_string(store.root().join(rel)).unwrap(),
		"package geom\n"
	);
}

#[test]
fn materialize_guard() {
	let (_dir, store) = new_repo();
	let session = "s2";
	store
		.write_blob(session, Path::new("a.txt"), b"one\n", "100644")
		.unwrap();
	let disk = store.root().join("a.txt");
	assert!(!disk.exists());
	let r1 = store.materialize(session, None, false).unwrap();
	assert_eq!(r1.applied, vec!["a.txt".to_owned()]);
	assert!(r1.conflicts.is_empty());
	assert_eq!(std::fs::read_to_string(&disk).unwrap(), "one\n");
	// Untouched on disk: re-materialize lands without a conflict.
	let r2 = store.materialize(session, None, false).unwrap();
	assert!(r2.conflicts.is_empty());
	// Edited on disk: the guard refuses. (Sleep first so the edit's mtime
	// can never share the materialized copy's millisecond tick.)
	std::thread::sleep(std::time::Duration::from_millis(10));
	std::fs::write(&disk, "two\n").unwrap();
	let r3 = store.materialize(session, None, false).unwrap();
	assert!(r3.applied.is_empty());
	assert!(r3.conflicts.iter().any(|(p, _)| p == "a.txt"));
	// force overrides the guard and restores the blob content.
	let r4 = store.materialize(session, None, true).unwrap();
	assert!(r4.conflicts.is_empty());
	assert_eq!(std::fs::read_to_string(&disk).unwrap(), "one\n");
}

#[test]
fn integrate_trailer_undo_and_no_remote() {
	let (_dir, store) = new_repo();
	let session = "s3";
	store
		.write_blob(session, Path::new("a.txt"), b"hello\n", "100644")
		.unwrap();
	let (tree, owned) = store.preview_integrate(session).unwrap();
	assert_eq!(owned, vec![('A', "a.txt".to_owned())]);
	let diff = store.tree_diff(&tree).unwrap();
	assert!(diff.contains("+hello"));
	let msg = "integrate one\n\nIntegrated-by: gitfs\nGitfs-Session: s3";
	let commit = store.commit_integrate(session, &tree, msg).unwrap();
	assert!(store.head_is_integrated().unwrap());
	let branch = store.branch_name().unwrap().unwrap();
	// No remote configured: push fails, the commit stays on the branch.
	assert!(store.push("origin", &branch).is_err());
	assert_eq!(store.head().unwrap().as_deref(), Some(commit.as_str()));
	// Undo rewinds the branch (and skips any remote rewind when none exists).
	let parent = store.undo("origin", &branch).unwrap();
	assert!(parent != commit);
	// Re-undo now refuses: HEAD is the init commit, not a gitfs integrate.
	let err = store.undo("origin", &branch).unwrap_err();
	assert!(err.contains("not a gitfs integrate"), "{err}");
}

#[test]
fn multi_path_overlay_survives_rewrite() {
	let (_dir, store) = new_repo();
	let session = "s4";
	store
		.write_blob(session, Path::new("keep.txt"), b"k\n", "100644")
		.unwrap();
	store
		.write_blob(session, Path::new("gone.txt"), b"g\n", "100644")
		.unwrap();
	let owned = store.owned_paths(session).unwrap();
	assert_eq!(owned.len(), 2);
	store.materialize(session, None, false).unwrap();
	assert_eq!(
		std::fs::read_to_string(store.root().join("gone.txt")).unwrap(),
		"g\n"
	);
	// A second write to one path does not disturb the other.
	store
		.write_blob(session, Path::new("keep.txt"), b"k2\n", "100644")
		.unwrap();
	store.materialize(session, None, false).unwrap();
	assert_eq!(
		std::fs::read_to_string(store.root().join("keep.txt")).unwrap(),
		"k2\n"
	);
	assert_eq!(
		std::fs::read_to_string(store.root().join("gone.txt")).unwrap(),
		"g\n"
	);
}

#[test]
fn unrelated_overlay_sessions_do_not_share_a_mutation_lock() {
	let (_dir, store) = new_repo();
	let held = store.mutation_lock("session:first").unwrap();
	let (tx, rx) = std::sync::mpsc::channel();
	let worker = std::thread::spawn(move || {
		tx.send(store.write_blob("second", Path::new("parallel.txt"), b"parallel", "100644"))
			.unwrap();
	});
	let result = rx.recv_timeout(std::time::Duration::from_secs(3));
	drop(held);
	worker.join().unwrap();
	assert!(result.unwrap().unwrap().is_some());
}

#[test]
fn concurrent_overlay_writes_keep_every_path() {
	let (_dir, store) = new_repo();
	std::thread::scope(|scope| {
		let workers: Vec<_> = (0..8)
			.map(|i| {
				let store = &store;
				scope.spawn(move || {
					store
						.write_blob("same", Path::new(&format!("file-{i}")), b"saved", "100644")
						.unwrap()
				})
			})
			.collect();
		for worker in workers {
			worker.join().unwrap();
		}
	});
	assert_eq!(store.owned_paths("same").unwrap().len(), 8);
}

#[test]
fn reviewed_publication_refuses_stale_head_without_deleting_new_unowned_files() {
	let (dir, store) = new_repo();
	store
		.write_blob("review", Path::new("owned"), b"overlay", "100644")
		.unwrap();
	let reviewed = store.reviewed_tree("review").unwrap();
	std::fs::write(dir.path().join("unowned"), "another actor").unwrap();
	store.git(&["add", "unowned"]).unwrap();
	store.git(&["commit", "-qm", "another actor"]).unwrap();
	let newer = store.head().unwrap();
	let failure = store
		.commit_reviewed(
			"review",
			&reviewed,
			"review",
			IDENT_NAME,
			IDENT_EMAIL,
			&AtomicBool::new(false),
		)
		.unwrap_err();
	assert!(failure.contains("changed"), "{failure}");
	assert_eq!(store.head().unwrap(), newer);
	assert_eq!(
		store.git_str(&["show", "HEAD:unowned"]).unwrap(),
		"another actor"
	);
	// A fresh review incorporates the new trunk file while retaining owned work.
	let fresh = store.reviewed_tree("review").unwrap();
	store
		.commit_reviewed(
			"review",
			&fresh,
			"fresh",
			IDENT_NAME,
			IDENT_EMAIL,
			&AtomicBool::new(false),
		)
		.unwrap();
	assert_eq!(
		store.git_str(&["show", "HEAD:unowned"]).unwrap(),
		"another actor"
	);
	assert_eq!(store.git_str(&["show", "HEAD:owned"]).unwrap(), "overlay");
}

#[test]
fn reviewed_publication_binds_index_overlay_branch_and_repository_identity() {
	for changed in [
		"index",
		"overlay",
		"branch",
		"repository",
		"cancel",
		"index-lock",
	] {
		let (dir, store) = new_repo();
		store
			.write_blob("review", Path::new("owned"), b"overlay", "100644")
			.unwrap();
		let mut reviewed = store.reviewed_tree("review").unwrap();
		match changed {
			"index" => {
				std::fs::write(dir.path().join("other"), "staged").unwrap();
				store.git(&["add", "other"]).unwrap();
			}
			"overlay" => {
				store
					.write_blob("review", Path::new("owned"), b"changed", "100644")
					.unwrap();
			}
			"branch" => {
				store.git(&["branch", "other"]).unwrap();
				store
					.git(&["symbolic-ref", "HEAD", "refs/heads/other"])
					.unwrap();
			}
			"repository" => reviewed.repository = "another-root".into(),
			"index-lock" => {
				std::fs::write(dir.path().join(".git/index.lock"), "another writer").unwrap()
			}
			_ => (),
		}
		let before = store.head().unwrap();
		let index = std::fs::read(dir.path().join(".git/index")).unwrap();
		assert!(
			store
				.commit_reviewed(
					"review",
					&reviewed,
					"review",
					IDENT_NAME,
					IDENT_EMAIL,
					&AtomicBool::new(changed == "cancel")
				)
				.is_err(),
			"{changed}"
		);
		assert_eq!(store.head().unwrap(), before, "{changed}");
		assert_eq!(
			std::fs::read(dir.path().join(".git/index")).unwrap(),
			index,
			"{changed}"
		);
		if changed == "index-lock" {
			assert_eq!(
				std::fs::read_to_string(dir.path().join(".git/index.lock")).unwrap(),
				"another writer"
			);
		}
	}
}

#[test]
fn concurrent_reviewed_publications_have_one_winner_and_preserve_worktree_and_index() {
	let (dir, store) = new_repo();
	store
		.write_blob("review", Path::new("owned"), b"overlay", "100644")
		.unwrap();
	std::fs::write(dir.path().join("README"), "user's unstaged edit").unwrap();
	let index = std::fs::read(dir.path().join(".git/index")).unwrap();
	let reviewed = store.reviewed_tree("review").unwrap();
	let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
	let handles: Vec<_> = (0..2)
		.map(|_| {
			let store = store.clone();
			let reviewed = reviewed.clone();
			let barrier = barrier.clone();
			std::thread::spawn(move || {
				barrier.wait();
				store.commit_reviewed(
					"review",
					&reviewed,
					"review\n\nIntegrated-by: gitfs",
					"Actual Author",
					"author@example.test",
					&AtomicBool::new(false),
				)
			})
		})
		.collect();
	barrier.wait();
	let winners = handles
		.into_iter()
		.map(|h| h.join().unwrap())
		.filter(Result::is_ok)
		.count();
	assert_eq!(winners, 1);
	assert_eq!(
		store
			.git_str(&["log", "-1", "--format=%an <%ae>|%cn <%ce>"])
			.unwrap()
			.trim(),
		"Actual Author <author@example.test>|kern gitfs <gitfs@kern.local>"
	);
	assert_eq!(std::fs::read(dir.path().join(".git/index")).unwrap(), index);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("README")).unwrap(),
		"user's unstaged edit"
	);
	assert!(!dir.path().join("owned").exists());
}

#[test]
fn prepared_head_transaction_holds_the_symbolic_branch_until_decision() {
	use std::io::{BufRead, BufReader, Write};
	let (_dir, store) = new_repo();
	store
		.write_blob("review", Path::new("owned"), b"overlay", "100644")
		.unwrap();
	let review = store.reviewed_tree("review").unwrap();
	store.git(&["branch", "other"]).unwrap();
	let mut process = git_cmd(store.root(), &["update-ref", "--stdin"], None)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();
	let mut input = process.stdin.take().unwrap();
	let mut output = BufReader::new(process.stdout.take().unwrap());
	write!(
		input,
		"start\nupdate HEAD {} {}\nverify {} {}\nprepare\n",
		review.overlay,
		review.head,
		store.branch("review"),
		review.overlay
	)
	.unwrap();
	input.flush().unwrap();
	for expected in ["start: ok", "prepare: ok"] {
		let mut line = String::new();
		output.read_line(&mut line).unwrap();
		assert_eq!(line.trim(), expected);
	}
	assert!(store
		.git(&["symbolic-ref", "HEAD", "refs/heads/other"])
		.is_err());
	assert!(store
		.git(&[
			"update-ref",
			&store.branch("review"),
			&review.head,
			&review.overlay
		])
		.is_err());
	assert_eq!(
		store.branch_name().unwrap().as_deref(),
		Some(review.branch.as_str())
	);
	input.write_all(b"abort\n").unwrap();
	drop(input);
	assert!(process.wait().unwrap().success());
	store
		.git(&["symbolic-ref", "HEAD", "refs/heads/other"])
		.unwrap();
	assert_eq!(store.head().unwrap().as_deref(), Some(review.head.as_str()));
}

#[test]
fn missing_publication_ack_remains_unknown_after_later_advance_or_failed_readback() {
	for head in [None, Some("old-head"), Some("later-advance")] {
		let error = publication_failure(true, "attempted", head, "lost acknowledgement");
		assert!(error.contains("outcome unknown"));
		assert!(error.contains("attempted"));
		assert!(!error.contains("refused"));
		assert!(!error.contains("preview again"));
	}
	assert!(
		publication_failure(true, "attempted", Some("attempted"), "lost ack")
			.contains("partial_success: published")
	);
	assert!(
		publication_failure(false, "attempted", None, "prepare failed")
			.contains("refused before dispatch")
	);
}

#[test]
fn integrate_carries_a_moved_trunk_but_refuses_a_read_behind_it() {
	let (dir, store) = new_repo();
	let git = |args: &[&str]| {
		let ok = std::process::Command::new("git")
			.arg("-C")
			.arg(dir.path())
			.args(args)
			.env("GIT_AUTHOR_NAME", "t")
			.env("GIT_AUTHOR_EMAIL", "t@t")
			.env("GIT_COMMITTER_NAME", "t")
			.env("GIT_COMMITTER_EMAIL", "t@t")
			.status()
			.unwrap()
			.success();
		assert!(ok, "git {args:?} failed");
	};
	let session = "s4";
	store
		.write_blob(session, Path::new("a.txt"), b"mine\n", "100644")
		.unwrap();
	// Trunk moves on another file: nothing this session read moved, so the
	// integrate tree carries both changes.
	std::fs::write(dir.path().join("b.txt"), "theirs\n").unwrap();
	git(&["add", "-A"]);
	git(&["commit", "-q", "-m", "trunk b"]);
	let (tree, owned) = store.preview_integrate(session).unwrap();
	assert_eq!(owned, vec![('A', "a.txt".to_owned())]);
	let listing = store.git_str(&["ls-tree", "--name-only", &tree]).unwrap();
	assert!(
		listing.contains("a.txt") && listing.contains("b.txt"),
		"{listing}"
	);
	// Trunk moves on the same file: the read this write was made against is
	// behind trunk, so integrate refuses and names the path, and nothing is merged
	// onto the session branch.
	let tip = store.tip(session).unwrap().unwrap();
	std::fs::write(dir.path().join("a.txt"), "trunk\n").unwrap();
	git(&["add", "-A"]);
	git(&["commit", "-q", "-m", "trunk a"]);
	let err = store.preview_integrate(session).unwrap_err();
	assert!(err.contains("read revision behind trunk in a.txt"), "{err}");
	assert_eq!(store.tip(session).unwrap().as_deref(), Some(tip.as_str()));
	let held = store.read(session, Path::new("a.txt")).unwrap().unwrap();
	assert_eq!(String::from_utf8(held).unwrap(), "mine\n");
	// Rolling the claim back drops the read, so a fresh read and write of
	// what trunk now holds integrates.
	let commits = store.session_commits(session, 50).unwrap();
	for commit in commits {
		let sha = commit["commit"].as_str().unwrap().to_owned();
		store
			.rollback_captured(session, &sha, &mut Capture::default())
			.unwrap();
	}
	assert!(store.owned_paths(session).unwrap().is_empty());
	store
		.write_blob(session, Path::new("a.txt"), b"trunk and mine\n", "100644")
		.unwrap();
	let (tree, owned) = store.preview_integrate(session).unwrap();
	assert_eq!(owned, vec![('A', "a.txt".to_owned())]);
	let integrated = store.git_str(&["show", &format!("{tree}:a.txt")]).unwrap();
	assert_eq!(integrated, "trunk and mine\n");
}

#[test]
fn a_stale_session_is_refused_while_the_session_that_integrated_first_lands() {
	let (_dir, store) = new_repo();
	// Two sessions read README at the same trunk revision and both write it.
	store
		.write_blob("first", Path::new("README"), b"first\n", "100644")
		.unwrap();
	store
		.write_blob("second", Path::new("README"), b"second\n", "100644")
		.unwrap();
	let (tree, _) = store.preview_integrate("first").unwrap();
	store
		.commit_integrate("first", &tree, "first lands")
		.unwrap();
	assert_eq!(store.git_str(&["show", "HEAD:README"]).unwrap(), "first\n");
	// The second read is now behind trunk on exactly that path.
	let err = store.preview_integrate("second").unwrap_err();
	assert!(
		err.contains("read revision behind trunk in README"),
		"{err}"
	);
	// A path neither session touched is not behind anything.
	store
		.write_blob("second", Path::new("other"), b"other\n", "100644")
		.unwrap();
	assert_eq!(
		store
			.stale_paths("second", &store.head().unwrap().unwrap())
			.unwrap(),
		vec!["README".to_owned()]
	);
}

#[test]
fn sync_integrated_updates_untouched_paths_and_keeps_dirty_ones() {
	let (dir, store) = new_repo();
	std::fs::write(dir.path().join("dirty"), "base\n").unwrap();
	store.git(&["add", "-A"]).unwrap();
	store.git(&["commit", "-qm", "more"]).unwrap();
	for (path, text) in [
		("README", "integrated\n"),
		("dirty", "integrated\n"),
		("new", "new\n"),
	] {
		store
			.write_blob("s", Path::new(path), text.as_bytes(), "100644")
			.unwrap();
	}
	std::fs::write(dir.path().join("dirty"), "user edit\n").unwrap();
	let review = store.reviewed_tree("s").unwrap();
	let commit = store
		.commit_reviewed(
			"s",
			&review,
			"integrate\n\nIntegrated-by: gitfs",
			"a",
			"a@b",
			&AtomicBool::new(false),
		)
		.unwrap();
	let sync = store
		.sync_integrated(&review.head, &commit, &review.owned)
		.unwrap();
	assert_eq!(sync["synced"], json!(["README", "new"]));
	assert_eq!(sync["kept"], json!(["dirty"]));
	assert_eq!(
		std::fs::read_to_string(dir.path().join("README")).unwrap(),
		"integrated\n"
	);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("new")).unwrap(),
		"new\n"
	);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("dirty")).unwrap(),
		"user edit\n"
	);
	// The integrated paths are clean; the kept one shows its index behind the
	// new HEAD and the user's edit behind the index.
	let status: Vec<String> = store
		.git_str(&["status", "--porcelain"])
		.unwrap()
		.lines()
		.filter(|l| !l.ends_with(".kern/"))
		.map(str::to_owned)
		.collect();
	assert_eq!(status, vec!["MM dirty".to_owned()]);
}

#[test]
fn trunk_read_serves_what_the_worktree_deleted_and_nothing_untracked() {
	let (dir, store) = new_repo();
	std::fs::remove_file(dir.path().join("README")).unwrap();
	assert_eq!(
		store.trunk_read(Path::new("README")).unwrap(),
		Some(b"base\n".to_vec())
	);
	assert_eq!(store.trunk_read(Path::new("never")).unwrap(), None);
}

#[test]
fn mode_for_keeps_the_executable_bit_before_the_first_write() {
	use std::os::unix::fs::PermissionsExt;
	let (dir, store) = new_repo();
	let hook = dir.path().join("hook");
	std::fs::write(&hook, "#!/bin/sh\n").unwrap();
	std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
	std::fs::write(dir.path().join("plain"), "x\n").unwrap();
	assert_eq!(store.mode_for("s1", Path::new("hook")).unwrap(), "100755");
	assert_eq!(store.mode_for("s1", Path::new("plain")).unwrap(), "100644");
	assert_eq!(store.mode_for("s1", Path::new("absent")).unwrap(), "100644");
	store
		.write_blob("s1", Path::new("hook"), b"#!/bin/sh\necho\n", "100755")
		.unwrap();
	std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o644)).unwrap();
	assert_eq!(
		store.mode_for("s1", Path::new("hook")).unwrap(),
		"100755",
		"session blob wins"
	);
}

#[test]
fn a_integrated_session_owns_nothing_and_integrates_again_after_trunk_moves() {
	let (dir, store) = new_repo();
	store
		.write_blob("twice", Path::new("a.txt"), b"one\n", "100644")
		.unwrap();
	let fresh = store.reviewed_tree("twice").unwrap();
	let integrated = store
		.commit_reviewed(
			"twice",
			&fresh,
			"first",
			IDENT_NAME,
			IDENT_EMAIL,
			&AtomicBool::new(false),
		)
		.unwrap();
	assert_eq!(
		store.tip("twice").unwrap().as_deref(),
		Some(integrated.as_str())
	);
	assert!(store.owned_paths("twice").unwrap().is_empty());
	// Trunk moves the integrated path; the session's next integrate is of another path.
	std::fs::write(dir.path().join("a.txt"), "two\n").unwrap();
	store.git(&["add", "a.txt"]).unwrap();
	store.git(&["commit", "-qm", "another actor"]).unwrap();
	store
		.write_blob("twice", Path::new("b.txt"), b"b\n", "100644")
		.unwrap();
	let second = store.reviewed_tree("twice").unwrap();
	store
		.commit_reviewed(
			"twice",
			&second,
			"second",
			IDENT_NAME,
			IDENT_EMAIL,
			&AtomicBool::new(false),
		)
		.unwrap();
	assert_eq!(store.git_str(&["show", "HEAD:a.txt"]).unwrap(), "two\n");
	assert_eq!(store.git_str(&["show", "HEAD:b.txt"]).unwrap(), "b\n");
}

#[test]
fn integrate_waits_out_a_brief_foreign_index_lock_and_refuses_a_held_one() {
	let dir = tempfile::tempdir().unwrap();
	let lock = dir.path().join("index.lock");
	std::fs::write(&lock, "").unwrap();
	let brief = lock.clone();
	let release = std::thread::spawn(move || {
		std::thread::sleep(std::time::Duration::from_millis(50));
		std::fs::remove_file(brief).unwrap();
	});
	let taken = IndexLock::take(lock.clone(), std::time::Duration::from_secs(2))
		.expect("a lock released within the patience is taken");
	release.join().unwrap();
	drop(taken);
	assert!(!lock.exists(), "dropping the guard removes the lock");
	std::fs::write(&lock, "").unwrap();
	let refused = IndexLock::take(lock.clone(), std::time::Duration::from_millis(30))
		.err()
		.expect("a lock held past the patience is refused");
	assert!(
		refused.starts_with("shared index busy; preview again"),
		"{refused}"
	);
	assert!(
		lock.exists(),
		"a refused take leaves the other holder's lock alone"
	);
}

#[test]
fn sync_integrated_waits_out_a_brief_foreign_index_lock() {
	let (dir, store) = new_repo();
	store
		.write_blob("s", Path::new("README"), b"integrated\n", "100644")
		.unwrap();
	let review = store.reviewed_tree("s").unwrap();
	let commit = store
		.commit_reviewed(
			"s",
			&review,
			"integrate\n\nIntegrated-by: gitfs",
			"a",
			"a@b",
			&AtomicBool::new(false),
		)
		.unwrap();
	let lock = dir.path().join(".git/index.lock");
	std::fs::write(&lock, "").unwrap();
	let held = lock.clone();
	let release = std::thread::spawn(move || {
		std::thread::sleep(std::time::Duration::from_millis(300));
		std::fs::remove_file(held).unwrap();
	});
	let sync = store
		.sync_integrated(&review.head, &commit, &review.owned)
		.unwrap();
	release.join().unwrap();
	assert_eq!(sync["synced"], json!(["README"]), "{sync}");
	assert_eq!(
		std::fs::read_to_string(dir.path().join("README")).unwrap(),
		"integrated\n"
	);
}

/// Commit `files` on trunk in the repo behind `store` (a deletion when the
/// content is `None`).
fn trunk_commit(store: &Store, files: &[(&str, Option<&str>)]) {
	for (rel, text) in files {
		let path = store.root().join(rel);
		match text {
			Some(text) => std::fs::write(&path, text).unwrap(),
			None => std::fs::remove_file(&path).unwrap(),
		}
	}
	let git = |args: &[&str]| {
		let ok = std::process::Command::new("git")
			.arg("-C")
			.arg(store.root())
			.args(args)
			.env("GIT_AUTHOR_NAME", "t")
			.env("GIT_AUTHOR_EMAIL", "t@t")
			.env("GIT_COMMITTER_NAME", "t")
			.env("GIT_COMMITTER_EMAIL", "t@t")
			.status()
			.unwrap()
			.success();
		assert!(ok, "git {args:?} failed");
	};
	git(&["add", "-A"]);
	git(&["commit", "-q", "-m", "trunk"]);
}

fn catchup(store: &Store, session: &str) -> Result<Value> {
	store.catchup_captured(session, &mut Capture::default())
}

#[test]
fn catchup_of_an_empty_or_current_session_is_a_no_op() {
	let (_dir, store) = new_repo();
	let empty = catchup(&store, "c0").unwrap();
	assert_eq!(empty["caught_up"], true, "{empty}");
	assert!(store.tip("c0").unwrap().is_none());
	store
		.write_blob("c0", Path::new("a.txt"), b"mine\n", "100644")
		.unwrap();
	let tip = store.tip("c0").unwrap();
	let current = catchup(&store, "c0").unwrap();
	assert_eq!(current["replayed"], json!([]), "{current}");
	assert_eq!(store.tip("c0").unwrap(), tip, "base == head writes nothing");
}

#[test]
fn catchup_replays_untouched_paths_onto_trunk_and_the_session_integrates() {
	let (_dir, store) = new_repo();
	store
		.write_blob("c1", Path::new("a.txt"), b"mine\n", "100644")
		.unwrap();
	trunk_commit(&store, &[("README", Some("trunk moved\n"))]);
	let head = store.head().unwrap().unwrap();
	let answer = catchup(&store, "c1").unwrap();
	assert_eq!(answer["replayed"], json!(["a.txt"]), "{answer}");
	assert_eq!(
		store.merge_base("c1").unwrap().as_deref(),
		Some(head.as_str())
	);
	assert_eq!(
		store.read("c1", Path::new("a.txt")).unwrap().unwrap(),
		b"mine\n"
	);
	assert_eq!(
		store.owned_paths("c1").unwrap(),
		vec![('A', "a.txt".to_owned())]
	);
	assert!(store.stale_paths("c1", &head).unwrap().is_empty());
	// Idempotent: a second catchup finds the session on trunk.
	let again = catchup(&store, "c1").unwrap();
	assert_eq!(again["base"], json!(head), "{again}");
	assert_eq!(again["replayed"], json!([]), "{again}");
}

#[test]
fn catchup_drops_what_trunk_already_holds_and_empties_the_session() {
	let (_dir, store) = new_repo();
	store
		.write_blob("c2", Path::new("README"), b"integrated\n", "100644")
		.unwrap();
	// Trunk lands the same content by another route (the stale-dylib
	// double-integrate): the session's change is superseded, not a conflict.
	trunk_commit(&store, &[("README", Some("integrated\n"))]);
	let answer = catchup(&store, "c2").unwrap();
	assert_eq!(answer["dropped"], json!(["README"]), "{answer}");
	assert_eq!(answer["ref_deleted"], true, "{answer}");
	assert!(store.tip("c2").unwrap().is_none());
	assert!(store.owned_paths("c2").unwrap().is_empty());
}

#[test]
fn catchup_refuses_a_real_conflict_by_path_and_writes_nothing() {
	let (_dir, store) = new_repo();
	store
		.write_blob("c3", Path::new("README"), b"mine\n", "100644")
		.unwrap();
	store
		.write_blob("c3", Path::new("b.txt"), b"b\n", "100644")
		.unwrap();
	let tip = store.tip("c3").unwrap();
	trunk_commit(&store, &[("README", Some("theirs\n"))]);
	let refused = catchup(&store, "c3").unwrap_err();
	assert!(refused.contains("trunk changed README"), "{refused}");
	assert!(refused.contains("rollback {path}"), "{refused}");
	assert!(!refused.contains("b.txt"), "{refused}");
	assert_eq!(store.tip("c3").unwrap(), tip, "atomic: nothing written");
}

#[test]
fn catchup_replays_a_session_deletion_trunk_did_not_touch() {
	let (_dir, store) = new_repo();
	store
		.write_blob("c4", Path::new("a.txt"), b"a\n", "100644")
		.unwrap();
	trunk_commit(&store, &[("keep.txt", Some("k\n"))]);
	// Delete README on the session, then trunk moves another path.
	store
		.commit_path_captured(
			"c4",
			Path::new("README"),
			None,
			&mut Capture::default(),
			Operation::Edit,
			"delete README",
		)
		.unwrap();
	trunk_commit(&store, &[("other.txt", Some("o\n"))]);
	let answer = catchup(&store, "c4").unwrap();
	let replayed = answer["replayed"].as_array().unwrap();
	assert!(replayed.contains(&json!("README")), "{answer}");
	assert!(store
		.owned_paths("c4")
		.unwrap()
		.contains(&('D', "README".to_owned())));
}

#[test]
fn catchup_refuses_a_ref_with_no_shared_history() {
	let (_dir, store) = new_repo();
	// The empty tree, committed with no parent: no history shared with trunk.
	let empty_tree = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
	let orphan = store
		.git_str(&["commit-tree", empty_tree, "-m", "orphan"])
		.unwrap();
	store
		.git(&["update-ref", "refs/gitfs/c5", orphan.trim()])
		.unwrap();
	let refused = catchup(&store, "c5").unwrap_err();
	assert!(refused.contains("shares no history"), "{refused}");
}
