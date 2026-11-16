//! Recovery contract through the real agent-neutral MCP server.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.current_dir(root)
		.args(args)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_string()
}

struct Mcp {
	child: Child,
	input: ChildStdin,
	output: BufReader<ChildStdout>,
	next: u64,
}

impl Mcp {
	fn new(root: &Path) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_git-fs"))
			.arg("mcp")
			.current_dir(root)
			.env("GIT_FS_DISK", root)
			.env("GIT_FS_SESSION", "recovery-test")
			.env("GIT_FS_CONFIG", "{}")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.unwrap();
		let input = child.stdin.take().unwrap();
		let output = BufReader::new(child.stdout.take().unwrap());
		let mut this = Self {
			child,
			input,
			output,
			next: 0,
		};
		this.request("initialize", json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"recovery","version":"1"}}));
		writeln!(
			this.input,
			"{}",
			json!({"jsonrpc":"2.0","method":"notifications/initialized"})
		)
		.unwrap();
		this
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		self.next += 1;
		writeln!(
			self.input,
			"{}",
			json!({"jsonrpc":"2.0","id":self.next,"method":method,"params":params})
		)
		.unwrap();
		self.input.flush().unwrap();
		let mut line = String::new();
		assert!(self.output.read_line(&mut line).unwrap() > 0);
		let result: Value = serde_json::from_str(&line).unwrap();
		assert_eq!(result["id"], self.next, "{result}");
		assert!(result.get("error").is_none(), "{result}");
		result["result"].clone()
	}

	fn call(&mut self, tool: &str, input: Value) -> Value {
		let result = self.request("tools/call", json!({"name":tool,"arguments":input}));
		assert_eq!(result["isError"], false, "{tool}: {result}");
		let text = result["content"][0]["text"].as_str().unwrap();
		serde_json::from_str(text).unwrap_or_else(|_| json!({"text":text}))
	}
}

impl Drop for Mcp {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

#[test]
fn committed_revision_reconciles_original_branch_after_switch() {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().canonicalize().unwrap();
	git(&root, &["init", "-q"]);
	git(&root, &["config", "user.name", "Test"]);
	git(&root, &["config", "user.email", "test@example.test"]);
	git(&root, &["config", "commit.gpgsign", "false"]);
	std::fs::write(root.join("README"), "base\\n").unwrap();
	git(&root, &["add", "README"]);
	git(&root, &["commit", "-qm", "base"]);

	let original_branch = git(&root, &["branch", "--show-current"]);
	let mut client = Mcp::new(&root);
	client.call("write", json!({"path":"a.txt","text":"mine\\n"}));
	let preview = client.call(
		"integrate",
		json!({"phase":"preview","message":"recover branch"}),
	);
	let revision = preview["revision"].as_str().unwrap().to_string();
	let first = client.call(
		"integrate",
		json!({"phase":"commit","expected_revision":revision,"message":"recover branch"}),
	);
	let original_commit = first["committed"].as_str().unwrap().to_string();
	let original_ref = format!("refs/heads/{original_branch}");
	assert_eq!(git(&root, &["rev-parse", &original_ref]), original_commit);
	assert_eq!(
		first["outcome"]["schema"], "gitfs-integration-outcome/v1",
		"{first}"
	);
	assert_eq!(first["outcome"]["publication"], "committed", "{first}");
	assert_eq!(
		first["outcome"]["synchronization"], "synchronized",
		"{first}"
	);
	let stage = git(&root, &["ls-files", "--stage", "--", "a.txt"]);
	let bytes = std::fs::read(root.join("a.txt")).unwrap();
	let assert_retry = |retry: Value| {
		assert_eq!(retry["committed"], original_commit, "{retry}");
		assert_eq!(retry["outcome"]["publication"], "committed", "{retry}");
		assert_eq!(
			retry["outcome"]["synchronization"], "synchronized",
			"{retry}"
		);
		assert_eq!(retry["worktree"]["synced"], json!(["a.txt"]), "{retry}");
		assert_eq!(retry["worktree"]["kept"], json!([]), "{retry}");
		assert_eq!(git(&root, &["ls-files", "--stage", "--", "a.txt"]), stage);
		assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), bytes);
	};
	assert_retry(client.call(
		"integrate",
		json!({"phase":"retry_sync","expected_revision":revision}),
	));
	drop(client);
	let mut client = Mcp::new(&root);
	assert_retry(client.call(
		"integrate",
		json!({"phase":"retry_sync","expected_revision":revision}),
	));

	let parent = git(&root, &["rev-parse", "HEAD^"]);
	git(&root, &["checkout", "-q", "-b", "other", &parent]);
	drop(client);
	let mut client = Mcp::new(&root);
	let reply = client.call(
		"integrate",
		json!({"phase":"reconcile","expected_revision":revision}),
	);
	assert_eq!(reply["integrated"], true, "{reply}");
	assert_eq!(reply["committed"], original_commit, "{reply}");
	assert_eq!(reply["branch"], original_branch, "{reply}");
	assert_eq!(
		reply["outcome"]["operation_id"], first["outcome"]["operation_id"],
		"{reply}"
	);
	assert_eq!(reply["outcome"]["publication"], "committed", "{reply}");
	assert_eq!(
		reply["outcome"]["synchronization"], "synchronized",
		"{reply}"
	);
	let refused = client.request(
		"tools/call",
		json!({
			"name":"integrate","arguments":{"phase":"retry_sync","expected_revision":revision}
		}),
	);
	assert_eq!(refused["isError"], true, "{refused}");
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), parent);
	assert_eq!(git(&root, &["rev-parse", &original_ref]), original_commit);
}

#[test]
fn deleting_a_path_is_synchronized_once_even_after_restarted_retries() {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().canonicalize().unwrap();
	git(&root, &["init", "-q"]);
	git(&root, &["config", "user.name", "Test"]);
	git(&root, &["config", "user.email", "test@example.test"]);
	git(&root, &["config", "commit.gpgsign", "false"]);
	std::fs::write(root.join("obsolete.txt"), "old\n").unwrap();
	git(&root, &["add", "obsolete.txt"]);
	git(&root, &["commit", "-qm", "base"]);
	let mut client = Mcp::new(&root);
	client.call("read", json!({"path":"obsolete.txt"}));
	client.call("delete", json!({"path":"obsolete.txt"}));
	let preview = client.call(
		"integrate",
		json!({"phase":"preview","message":"remove obsolete"}),
	);
	let revision = preview["revision"].as_str().unwrap().to_string();
	let first = client.call(
		"integrate",
		json!({"phase":"commit","expected_revision":revision,"message":"remove obsolete"}),
	);
	let committed = first["committed"].as_str().unwrap().to_string();
	assert_eq!(
		first["outcome"]["synchronization"], "synchronized",
		"{first}"
	);
	let verify = |receipt: Value| {
		assert_eq!(receipt["committed"], committed, "{receipt}");
		assert_eq!(
			receipt["outcome"]["synchronization"], "synchronized",
			"{receipt}"
		);
		assert_eq!(
			receipt["worktree"]["synced"],
			json!(["obsolete.txt"]),
			"{receipt}"
		);
		assert_eq!(receipt["worktree"]["kept"], json!([]), "{receipt}");
		assert!(!root.join("obsolete.txt").exists());
		assert_eq!(git(&root, &["ls-files", "--", "obsolete.txt"]), "");
		assert_eq!(git(&root, &["rev-parse", "HEAD"]), committed);
	};
	verify(client.call(
		"integrate",
		json!({"phase":"retry_sync","expected_revision":revision}),
	));
	drop(client);
	let mut client = Mcp::new(&root);
	verify(client.call(
		"integrate",
		json!({"phase":"retry_sync","expected_revision":revision}),
	));
	std::fs::write(root.join("obsolete.txt"), "foreign\\n").unwrap();
	let kept = client.call(
		"integrate",
		json!({"phase":"retry_sync","expected_revision":revision}),
	);
	assert_eq!(kept["committed"], committed, "{kept}");
	assert_eq!(kept["outcome"]["synchronization"], "kept", "{kept}");
	assert_eq!(kept["worktree"]["kept"], json!(["obsolete.txt"]), "{kept}");
	assert_eq!(git(&root, &["ls-files", "--", "obsolete.txt"]), "");
	assert_eq!(
		std::fs::read_to_string(root.join("obsolete.txt")).unwrap(),
		"foreign\\n"
	);
}
