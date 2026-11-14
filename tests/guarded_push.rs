//! Exercise the real private hook, including Git for Windows' executable hook.
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.current_dir(root)
		.args([
			"-c",
			"user.name=Test",
			"-c",
			"user.email=test@example.com",
			"-c",
			"commit.gpgsign=false",
		])
		.args(args)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().into()
}

struct Fixture {
	_temp: tempfile::TempDir,
	root: PathBuf,
	remote: PathBuf,
	old: String,
	new: String,
}

impl Fixture {
	fn new() -> Self {
		let temp = tempfile::tempdir().unwrap();
		let root = temp.path().join("working copy");
		let remote = temp.path().join("remote repository.git");
		git(
			temp.path(),
			&[
				"init",
				"-q",
				"--bare",
				"-b",
				"main",
				remote.to_str().unwrap(),
			],
		);
		git(
			temp.path(),
			&["init", "-q", "-b", "main", root.to_str().unwrap()],
		);
		fs::write(root.join("tracked.txt"), "base\n").unwrap();
		git(&root, &["add", "tracked.txt"]);
		git(&root, &["commit", "-qm", "base"]);
		git(
			&root,
			&["remote", "add", "origin", remote.to_str().unwrap()],
		);
		git(&root, &["push", "-qu", "origin", "main"]);
		let old = git(&root, &["rev-parse", "HEAD"]);
		let receipt = format!(
			"integrated\n\nIntegrated-by: gitfs\nGitfs-Session: push-test\nGitfs-Review: {}\nGitfs-Consumed: {}",
			"a".repeat(64), "b".repeat(40),
		);
		git(&root, &["commit", "-q", "--allow-empty", "-m", &receipt]);
		let new = git(&root, &["rev-parse", "HEAD"]);
		Self {
			_temp: temp,
			root,
			remote,
			old,
			new,
		}
	}

	fn call(&self, input: Value) -> Value {
		self.call_as("push-test", input)
	}

	fn call_as(&self, session: &str, input: Value) -> Value {
		let output = Command::new(env!("CARGO_BIN_EXE_git-fs"))
			.current_dir(&self.root)
			.env("GIT_FS_DISK", &self.root)
			.env("GIT_FS_SESSION", session)
			.env("GIT_FS_CONFIG", "{}")
			.args(["call", "push", &input.to_string()])
			.output()
			.unwrap();
		let result: Value = serde_json::from_slice(&output.stdout)
			.unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stderr)));
		if result["error"] == true {
			return result;
		}
		match &result["content"] {
			Value::String(text) => serde_json::from_str(text).unwrap(),
			value => value.clone(),
		}
	}

	fn commit_input(&self, id: &str) -> Value {
		let mut input = json!({"phase":"preview", "remote":"origin", "ref":"refs/heads/main",
			"commit":self.new, "expected_remote_head":self.old});
		let preview = self.call(input.clone());
		assert_eq!(preview["phase"], "preview", "{preview}");
		input["phase"] = json!("commit");
		input["expected_revision"] = preview["revision"].clone();
		input["operation_id"] = json!(id);
		input
	}

	fn handoff_input(&self, id: &str, recipient: &str) -> Value {
		let mut input = json!({"phase":"preview", "remote":"origin", "ref":"refs/heads/main",
			"commit":self.new, "expected_remote_head":self.old, "recipient_session":recipient});
		let preview = self.call(input.clone());
		assert_eq!(preview["next_phase"], "handoff", "{preview}");
		input["phase"] = json!("handoff");
		input["expected_revision"] = preview["revision"].clone();
		input["operation_id"] = json!(id);
		input
	}

	fn hook(&self, exit: u8) -> PathBuf {
		let hooks = self.root.join("custom hooks");
		fs::create_dir(&hooks).unwrap();
		git(
			&self.root,
			&["config", "core.hooksPath", hooks.to_str().unwrap()],
		);
		let hook = hooks.join("pre-push");
		fs::write(&hook, format!(
			"#!/bin/sh\nprintf '%s\\n' \"$1\" \"$2\" > hook-args\ncat > hook-input-seen\nprintf 'run\\n' >> hook-runs\nexit {exit}\n"
		)).unwrap();
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
		}
		hook
	}
}

fn configured_hooks_supported(root: &Path) -> bool {
	// Invocation-only configuration leaves no probe entries in the repository.
	let output = Command::new("git")
		.current_dir(root)
		.args([
			"-c",
			&format!(
				"core.hooksPath={}",
				root.join("absent-probe-hooks").display()
			),
			"-c",
			"hook.git-fs-probe.event=pre-push",
			"-c",
			"hook.git-fs-probe.command=echo git-fs-configured-hook-probe",
			"hook",
			"run",
			"--ignore-missing",
			"pre-push",
		])
		.output()
		.unwrap();
	let supported = output.status.success()
		&& [output.stdout, output.stderr]
			.iter()
			.any(|s| String::from_utf8_lossy(s).contains("git-fs-configured-hook-probe"));
	if !supported {
		eprintln!("Git lacks configured hooks; skipping configured-hook assertions.");
	}
	supported
}

#[test]
fn guarded_push_runs_original_hook_preserves_index_and_does_not_replay() {
	let f = Fixture::new();
	let configured_hooks = configured_hooks_supported(&f.root);
	let hook = f.hook(0);
	let original_hook = fs::read(&hook).unwrap();
	if configured_hooks {
		git(&f.root, &["config", "hook.audit.event", "pre-push"]);
		git(
			&f.root,
			&[
				"config",
				"hook.audit.command",
				"printf 'configured\\n' >> configured-runs",
			],
		);
		git(&f.root, &["config", "hook.disabled.event", "pre-push"]);
		git(&f.root, &["config", "hook.disabled.command", "exit 1"]);
		git(&f.root, &["config", "hook.disabled.enabled", "false"]);
	}
	fs::write(f.root.join("staged.txt"), "someone else's staged content\n").unwrap();
	git(&f.root, &["add", "staged.txt"]);
	fs::write(
		f.root.join("tracked.txt"),
		"someone else's unstaged content\n",
	)
	.unwrap();
	let index = fs::read(f.root.join(".git/index")).unwrap();
	let input = f.commit_input("success");
	let result = f.call(input.clone());
	assert_eq!(result["status"], "confirmed", "{result}");
	assert_eq!(git(&f.remote, &["rev-parse", "refs/heads/main"]), f.new);
	assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index);
	assert_eq!(fs::read(&hook).unwrap(), original_hook);
	assert_eq!(
		git(&f.root, &["config", "--get", "core.hooksPath"]),
		hook.parent().unwrap().to_str().unwrap()
	);
	assert_eq!(
		fs::read_to_string(f.root.join("tracked.txt")).unwrap(),
		"someone else's unstaged content\n"
	);
	let arguments = fs::read_to_string(f.root.join("hook-args")).unwrap();
	assert_eq!(
		arguments.lines().collect::<Vec<_>>(),
		["origin", f.remote.to_str().unwrap()]
	);
	assert_eq!(
		fs::read_to_string(f.root.join("hook-input-seen")).unwrap(),
		format!("{} {} refs/heads/main {}\n", f.new, f.new, f.old)
	);
	assert_eq!(f.call(input), result);
	assert_eq!(
		f.call(json!({"phase":"reconcile", "operation_id":"success"})),
		result
	);
	assert_eq!(
		fs::read_to_string(f.root.join("hook-runs")).unwrap(),
		"run\n"
	);
	if configured_hooks {
		assert_eq!(
			fs::read_to_string(f.root.join("configured-runs")).unwrap(),
			"configured\n"
		);
	}
}

#[test]
fn configured_hook_veto_is_recorded_without_running_twice() {
	let f = Fixture::new();
	if !configured_hooks_supported(&f.root) {
		return;
	}
	git(&f.root, &["config", "hook.veto.event", "pre-push"]);
	git(
		&f.root,
		&[
			"config",
			"hook.veto.command",
			"printf 'veto\\n' >> configured-runs; exit 1",
		],
	);
	let input = f.commit_input("configured-refusal");
	let result = f.call(input.clone());
	assert_eq!(result["status"], "refused", "{result}");
	assert_eq!(git(&f.remote, &["rev-parse", "refs/heads/main"]), f.old);
	assert_eq!(f.call(input), result);
	assert_eq!(
		fs::read_to_string(f.root.join("configured-runs")).unwrap(),
		"veto\n"
	);
}

#[test]
fn legacy_hook_enable_setting_matches_native_git() {
	let f = Fixture::new();
	f.hook(1);
	git(&f.root, &["config", "hook.pre-push.enabled", "false"]);
	// Older Git ignores this setting for legacy hooks; match installed Git.
	let baseline = Command::new("git")
		.current_dir(&f.root)
		.args([
			"hook",
			"run",
			"--ignore-missing",
			"pre-push",
			"--",
			"origin",
			f.remote.to_str().unwrap(),
		])
		.output()
		.unwrap()
		.status
		.success();
	for name in ["hook-runs", "hook-args", "hook-input-seen"] {
		let _ = fs::remove_file(f.root.join(name));
	}
	let result = f.call(f.commit_input("disabled-event"));
	assert_eq!(
		result["status"],
		if baseline { "confirmed" } else { "refused" },
		"{result}"
	);
	assert_eq!(
		git(&f.remote, &["rev-parse", "refs/heads/main"]),
		if baseline { f.new } else { f.old }
	);
	assert_eq!(f.root.join("hook-runs").exists(), !baseline);
	assert_eq!(
		git(&f.root, &["config", "--get", "hook.pre-push.enabled"]),
		"false"
	);
}

#[test]
fn original_hook_veto_is_durable_and_never_moves_remote() {
	let f = Fixture::new();
	f.hook(1);
	let input = f.commit_input("refusal");
	let result = f.call(input.clone());
	assert_eq!(result["status"], "refused", "{result}");
	assert_eq!(result["reason"], "hook_refused", "{result}");
	assert_eq!(git(&f.remote, &["rev-parse", "refs/heads/main"]), f.old);
	assert_eq!(f.call(input), result);
	assert_eq!(
		fs::read_to_string(f.root.join("hook-runs")).unwrap(),
		"run\n"
	);
}

#[test]
fn changed_remote_after_preview_is_refused_before_dispatch() {
	let f = Fixture::new();
	let input = f.commit_input("stale");
	git(&f.root, &["commit", "-q", "--allow-empty", "-m", "later"]);
	git(&f.root, &["push", "-q", "origin", "main"]);
	let current = git(&f.remote, &["rev-parse", "refs/heads/main"]);
	f.hook(0);
	let result = f.call(input);
	assert_eq!(result["error"], true, "{result}");
	assert!(
		result["content"]
			.as_str()
			.unwrap()
			.contains("expected remote head changed"),
		"{result}"
	);
	assert!(!f.root.join("hook-runs").exists());
	assert_eq!(git(&f.remote, &["rev-parse", "refs/heads/main"]), current);
}

fn recipient_commit(record: &Value) -> Value {
	let b = &record["binding"];
	json!({"phase":"commit","operation_id":record["operation_id"],"expected_revision":record["revision"],
		"remote":b["remote"],"ref":b["reference"],"commit":b["commit"],"expected_remote_head":b["expected_remote_head"]})
}

#[test]
fn source_authorized_handoff_survives_restart_and_dispatches_once() {
	let f = Fixture::new();
	f.hook(0);
	let input = f.handoff_input("delivery", "recovery");
	let grant = f.call(input.clone());
	assert_eq!(grant["status"], "ready", "{grant}");
	assert_eq!(grant["binding"]["session"], "recovery");
	assert_eq!(grant["binding"]["authorized_by"], "push-test");
	assert_eq!(f.call(input), grant);
	assert_eq!(git(&f.remote, &["rev-parse", "main"]), f.old);
	let recovered = f.call_as(
		"recovery",
		json!({"phase":"reconcile","operation_id":"delivery"}),
	);
	assert_eq!(recovered["status"], "ready", "{recovered}");
	let commit = recipient_commit(&recovered);
	let (a, b) = std::thread::scope(|scope| {
		let first = scope.spawn(|| f.call_as("recovery", commit.clone()));
		let second = scope.spawn(|| f.call_as("recovery", commit.clone()));
		(first.join().unwrap(), second.join().unwrap())
	});
	for r in [&a, &b] {
		assert!(
			r["status"] == "confirmed" || r["status"] == "unknown",
			"{r}"
		);
	}
	assert_eq!(git(&f.remote, &["rev-parse", "main"]), f.new);
	assert_eq!(f.call_as("recovery", commit)["status"], "confirmed");
	assert_eq!(
		fs::read_to_string(f.root.join("hook-runs"))
			.unwrap()
			.lines()
			.count(),
		1
	);
}

#[test]
fn push_handoff_requires_source_authority_and_exact_recipient_binding() {
	let f = Fixture::new();
	let bare = json!({"remote":"origin","ref":"refs/heads/main","commit":f.new,"expected_remote_head":f.old});
	assert_eq!(f.call_as("recovery", bare.clone())["error"], true);
	let input = f.handoff_input("delivery", "recovery");
	assert_eq!(f.call_as("intruder", input.clone())["error"], true);
	let mut stale = input.clone();
	stale["expected_revision"] = json!("0".repeat(64));
	assert_eq!(f.call(stale)["error"], true);
	let grant = f.call(input);
	let commit = recipient_commit(&grant);
	assert_eq!(f.call_as("intruder", commit.clone())["error"], true);
	let missing_reviewer = f.root.join("missing-reviewer.exe");
	let policy = json!([missing_reviewer]).to_string();
	git(&f.root, &["config", "gitfs.reviewCommand", &policy]);
	assert_eq!(f.call_as("recovery", commit.clone())["error"], true);
	git(&f.root, &["config", "--unset", "gitfs.reviewCommand"]);
	let mut wrong = commit.clone();
	wrong["ref"] = json!("refs/heads/other");
	assert_eq!(f.call_as("recovery", wrong)["error"], true);
	git(
		&f.root,
		&["config", "remote.origin.url", f.root.to_str().unwrap()],
	);
	assert_eq!(f.call_as("recovery", commit.clone())["error"], true);
	git(
		&f.root,
		&["config", "remote.origin.url", f.remote.to_str().unwrap()],
	);
	git(
		&f.root,
		&["commit", "-q", "--allow-empty", "-m", "later local work"],
	);
	assert_eq!(f.call_as("recovery", commit)["error"], true);
	assert_eq!(git(&f.remote, &["rev-parse", "main"]), f.old);
}

#[test]
fn push_handoff_preserves_hook_veto_without_retrying_dispatch() {
	let f = Fixture::new();
	f.hook(1);
	let grant = f.call(f.handoff_input("delivery", "recovery"));
	let input = recipient_commit(&grant);
	let refused = f.call_as("recovery", input.clone());
	assert_eq!(refused["status"], "refused", "{refused}");
	assert_eq!(refused["reason"], "hook_refused");
	assert_eq!(f.call_as("recovery", input), refused);
	assert_eq!(
		fs::read_to_string(f.root.join("hook-runs"))
			.unwrap()
			.lines()
			.count(),
		1
	);
	assert_eq!(git(&f.remote, &["rev-parse", "main"]), f.old);
}

#[test]
fn push_handoff_refuses_remote_drift_and_reconciles_exact_delivery() {
	let f = Fixture::new();
	let grant = f.call(f.handoff_input("delivery", "recovery"));
	let tree = git(&f.root, &["rev-parse", "HEAD^{tree}"]);
	let competing = git(
		&f.root,
		&[
			"commit-tree",
			&tree,
			"-p",
			&f.old,
			"-m",
			"competing delivery",
		],
	);
	git(
		&f.root,
		&[
			"push",
			"-q",
			"origin",
			&format!("{competing}:refs/heads/main"),
		],
	);
	let answer = f.call_as("recovery", recipient_commit(&grant));
	assert_eq!(answer["error"], true, "{answer}");
	assert_eq!(git(&f.remote, &["rev-parse", "main"]), competing);
	let g = Fixture::new();
	g.call(g.handoff_input("delivery", "recovery"));
	git(
		&g.root,
		&[
			"push",
			"-q",
			"origin",
			&format!("{}:refs/heads/main", g.new),
		],
	);
	let confirmed = g.call_as(
		"recovery",
		json!({"phase":"reconcile","operation_id":"delivery"}),
	);
	assert_eq!(confirmed["status"], "confirmed", "{confirmed}");
	assert!(!g.root.join("hook-runs").exists());
}
