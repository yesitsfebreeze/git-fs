//! Executable protocol tests: real git repositories, hook stdin/stdout, MCP.
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn repo() -> (tempfile::TempDir, PathBuf) {
	let temp = tempfile::tempdir().unwrap();
	let root = temp.path().canonicalize().unwrap();
	for args in [
		&["init", "-q"][..],
		&["config", "user.name", "Test"],
		&["config", "user.email", "test@example.com"],
	] {
		assert!(Command::new("git")
			.args(args)
			.current_dir(&root)
			.status()
			.unwrap()
			.success());
	}
	std::fs::write(root.join("a.txt"), "base\n").unwrap();
	assert!(Command::new("git")
		.args(["add", "."])
		.current_dir(&root)
		.status()
		.unwrap()
		.success());
	assert!(Command::new("git")
		.args(["commit", "-qm", "base"])
		.current_dir(&root)
		.status()
		.unwrap()
		.success());
	(temp, root)
}

fn invoke(root: &Path, args: &[&str], input: &str, local: bool) -> String {
	let mut child = Command::new(env!("CARGO_BIN_EXE_git-fs"))
		.args(args)
		.current_dir(root)
		.env(
			"GIT_FS_CONFIG",
			if local { r#"{"mode":"local"}"# } else { "{}" },
		)
		.env_remove("GIT_FS_DISK")
		.env_remove("GIT_FS_SESSION")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();
	child
		.stdin
		.take()
		.unwrap()
		.write_all(input.as_bytes())
		.unwrap();
	let out = child.wait_with_output().unwrap();
	assert!(
		out.status.success(),
		"{}",
		String::from_utf8_lossy(&out.stderr)
	);
	String::from_utf8(out.stdout).unwrap()
}

fn hook(root: &Path, action: &str, session: &str, tool: &str, input: Value, local: bool) -> Value {
	let output = invoke(
		root,
		&["hook-codex", action],
		&json!({"session_id":session,"cwd":root,"tool_name":tool,"tool_input":input}).to_string(),
		local,
	);
	serde_json::from_str(&output).unwrap_or(Value::Null)
}

fn request(root: &Path, session: &str, name: &str, args: Value, id: usize) -> Value {
	let routed = hook(
		root,
		"pre-tool",
		session,
		&format!("mcp__git-fs__{name}"),
		args,
		false,
	);
	json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":routed["hookSpecificOutput"]["updatedInput"]}})
}

fn mcp(root: &Path, requests: Vec<Value>) -> Vec<Value> {
	let input = requests
		.iter()
		.map(|v| format!("{v}\n"))
		.collect::<String>();
	invoke(root, &["mcp", "codex"], &input, false)
		.lines()
		.map(|s| serde_json::from_str(s).unwrap())
		.collect()
}

#[test]
fn lifecycle_routes_two_sessions_and_materializes_only_the_stopped_one() {
	let (_temp, root) = repo();
	for s in ["first", "second"] {
		let guide = hook(&root, "session-start", s, "", json!({}), false);
		assert!(guide["hookSpecificOutput"]["additionalContext"]
			.as_str()
			.unwrap()
			.contains(&format!("refs/gitfs/{s}")));
	}
	assert!(!root.join(".git-fs/CURRENT").exists());
	let answers = mcp(
		&root,
		vec![
			request(&root, "first", "read", json!({"path":"a.txt"}), 1),
			request(
				&root,
				"first",
				"write",
				json!({"path":"a.txt","text":"first\n"}),
				2,
			),
			request(&root, "second", "read", json!({"path":"a.txt"}), 3),
			request(
				&root,
				"second",
				"write",
				json!({"path":"a.txt","text":"second\n"}),
				4,
			),
			request(&root, "first", "read", json!({"path":"a.txt"}), 5),
		],
	);
	for answer in &answers {
		assert_eq!(answer["result"]["isError"], false, "{answer}");
	}
	assert!(answers[4]["result"]["content"][0]["text"]
		.as_str()
		.unwrap()
		.contains("first"));
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"base\n"
	);
	hook(&root, "stop", "first", "", json!({}), false);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"first\n"
	);
	// Existing overwrite guards keep the second session from overwriting the first.
	// Git commit timestamps have one-second resolution; make the disk change
	// unambiguously newer instead of depending on which second this test runs.
	std::fs::File::open(root.join("a.txt"))
		.unwrap()
		.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2))
		.unwrap();
	hook(&root, "stop", "second", "", json!({}), false);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"first\n"
	);
}

#[test]
fn patches_are_denied_inside_allowed_outside_and_captured_in_local_mode() {
	let (_temp, root) = repo();
	let patch = json!({"command":"*** Begin Patch\n*** Update File: a.txt\n@@\n-base\n+native\n*** Add File: new.txt\n+new\n*** End Patch"});
	assert_eq!(
		hook(&root, "pre-tool", "s", "apply_patch", patch.clone(), false)["hookSpecificOutput"]
			["permissionDecision"],
		"deny"
	);
	assert!(hook(
		&root,
		"pre-tool",
		"s",
		"apply_patch",
		json!({"command":"*** Begin Patch\n*** Add File: /outside/x\n+x\n*** End Patch"}),
		false
	)
	.is_null());
	assert!(hook(&root, "pre-tool", "s", "apply_patch", patch.clone(), true).is_null());
	std::fs::write(root.join("a.txt"), "native\n").unwrap();
	std::fs::write(root.join("new.txt"), "new\n").unwrap();
	hook(&root, "post-edit", "s", "apply_patch", patch, true);
	let store = gitfs::store::StoreConfig::new(".git-fs").at(&root).unwrap();
	assert_eq!(
		store.read("s", Path::new("a.txt")).unwrap().unwrap(),
		b"native\n"
	);
	assert_eq!(
		store.read("s", Path::new("new.txt")).unwrap().unwrap(),
		b"new\n"
	);
}

#[test]
fn mcp_uses_hook_workspace_and_refuses_unrouted_calls() {
	let (_temp, root) = repo();
	let (_other_temp, other) = repo();
	std::fs::write(other.join("other.txt"), "only other workspace\n").unwrap();
	let answers = mcp(
		&root,
		vec![
			request(
				&other,
				"other-session",
				"read",
				json!({"path":"other.txt"}),
				1,
			),
			json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"gitfs_ls","arguments":{}}}),
		],
	);
	assert_eq!(answers[0]["result"]["isError"], false);
	assert!(answers[0]["result"]["content"][0]["text"]
		.as_str()
		.unwrap()
		.contains("only other workspace"));
	assert_eq!(answers[1]["result"]["isError"], true);
	assert!(answers[1]["result"]["content"][0]["text"]
		.as_str()
		.unwrap()
		.contains("/hooks"));
}

#[test]
fn shell_guidance_and_non_repository_calls_preserve_existing_behavior() {
	let (_temp, root) = repo();
	assert!(hook(
		&root,
		"pre-tool",
		"s",
		"Bash",
		json!({"command":"rg needle"}),
		false
	)["hookSpecificOutput"]["additionalContext"]
		.as_str()
		.unwrap()
		.contains("git-fs grep"));
	assert!(hook(
		&root,
		"pre-tool",
		"s",
		"Bash",
		json!({"command":"cargo test"}),
		false
	)
	.is_null());
	let outside = tempfile::tempdir().unwrap();
	assert!(hook(outside.path(), "session-start", "s", "", json!({}), false).is_null());
}
