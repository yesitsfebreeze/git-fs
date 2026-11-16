//! The native MCP contract must expose the verified candidate checkout API.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct Server {
	child: Child,
	input: ChildStdin,
	output: Receiver<String>,
	reader: Option<JoinHandle<()>>,
	next_id: u64,
}

impl Server {
	fn new(root: &Path, session: &str) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_git-fs"))
			.arg("mcp")
			.current_dir(root)
			.env("GIT_FS_DISK", root)
			.env("GIT_FS_SESSION", session)
			.env("GIT_FS_CONFIG", "{}")
			.env("GIT_FS_COORDINATED", "1")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.unwrap();
		let input = child.stdin.take().unwrap();
		let output = child.stdout.take().unwrap();
		let (sender, receiver) = mpsc::channel();
		let reader = thread::spawn(move || {
			let mut output = BufReader::new(output);
			loop {
				let mut line = String::new();
				match output.read_line(&mut line) {
					Ok(0) | Err(_) => break,
					Ok(_) if sender.send(line).is_err() => break,
					Ok(_) => {}
				}
			}
		});
		let mut server = Self {
			child,
			input,
			output: receiver,
			reader: Some(reader),
			next_id: 0,
		};
		server.request(
			"initialize",
			json!({
				"protocolVersion": "2025-06-18",
				"capabilities": {},
				"clientInfo": {"name": "candidate-contract", "version": "1"}
			}),
		);
		writeln!(
			server.input,
			"{}",
			json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
		)
		.unwrap();
		server.input.flush().unwrap();
		server
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		self.next_id += 1;
		let operation = match params["name"].as_str() {
			Some(name) if method == "tools/call" => name.to_owned(),
			_ => method.to_owned(),
		};
		writeln!(
			self.input,
			"{}",
			json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
		)
		.unwrap();
		self.input.flush().unwrap();
		let line = self
			.output
			.recv_timeout(Duration::from_secs(20))
			.unwrap_or_else(|error| {
				panic!(
					"native MCP {operation} id={} child={} reply failure: {error}",
					self.next_id,
					self.child.id()
				)
			});
		let reply: Value = serde_json::from_str(&line).unwrap();
		assert_eq!(reply["jsonrpc"], "2.0");
		assert_eq!(reply["id"], self.next_id);
		assert!(reply.get("error").is_none(), "{reply}");
		reply["result"].clone()
	}

	fn call(&mut self, tool: &str, arguments: Value) -> Value {
		self.request("tools/call", json!({"name": tool, "arguments": arguments}))
	}

	fn ok(&mut self, tool: &str, arguments: Value) -> Value {
		let reply = self.call(tool, arguments);
		assert_eq!(reply["isError"], false, "{tool}: {reply}");
		reply
	}
}

impl Drop for Server {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
		if let Some(reader) = self.reader.take() {
			if reader.is_finished() {
				let _ = reader.join();
			}
		}
	}
}

fn rejects(reply: &Value, reason: &str) {
	assert_eq!(reply["isError"], true, "{reply}");
	let message = reply["content"][0]["text"]
		.as_str()
		.expect("MCP error text");
	assert!(
		message.contains(reason),
		"expected {reason:?}, got {message}"
	);
}

fn content(reply: &Value) -> Value {
	serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.args(args)
		.current_dir(root)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fixture() -> tempfile::TempDir {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q", "-b", "main"]);
	git(dir.path(), &["config", "user.name", "candidate-contract"]);
	git(
		dir.path(),
		&["config", "user.email", "candidate@example.invalid"],
	);
	git(dir.path(), &["config", "commit.gpgsign", "false"]);
	std::fs::write(dir.path().join("file.txt"), "before\n").unwrap();
	std::fs::write(dir.path().join(".gitignore"), ".git-fs/\n").unwrap();
	git(dir.path(), &["add", "."]);
	git(dir.path(), &["commit", "-qm", "base"]);
	dir
}

#[test]
fn native_mcp_candidate_snapshot_schema_and_release_are_executable() {
	let dir = fixture();
	let root = dir.path().canonicalize().unwrap();
	let before_head = git(&root, &["rev-parse", "HEAD"]);
	let before_index = std::fs::read(root.join(".git/index")).unwrap();
	let mut a = Server::new(&root, "candidate-contract-a");
	let mut b = Server::new(&root, "candidate-contract-b");

	let list = a.request("tools/list", json!({}));
	let tool = list["tools"]
		.as_array()
		.unwrap()
		.iter()
		.find(|item| item["name"] == "gitfs_snapshot")
		.unwrap();
	let schema = &tool["inputSchema"];
	let fields = &schema["properties"];
	assert_eq!(fields["kind"]["type"], "string", "{schema}");
	assert_eq!(fields["expected_tree"]["type"], "string", "{schema}");
	assert_eq!(fields["expected_review"]["type"], "object", "{schema}");
	assert_eq!(fields["capability"]["type"], "string", "{schema}");
	assert_eq!(schema["additionalProperties"], false, "{schema}");

	a.ok("read", json!({"path": "file.txt"}));
	a.ok(
		"edit",
		json!({"path": "file.txt", "old": "before", "new": "candidate"}),
	);
	let preview = content(&a.ok(
		"integrate",
		json!({"phase": "preview", "message": "candidate"}),
	));
	let reviewed = preview["tree"]["source"].clone();
	assert!(
		reviewed.is_object() && reviewed["tree"].is_string(),
		"{preview}"
	);

	let snapshot = content(&a.ok(
		"gitfs_snapshot",
		json!({
			"kind": "candidate",
			"expected_tree": reviewed["tree"],
			"expected_review": reviewed
		}),
	));
	let checkout = PathBuf::from(snapshot["path"].as_str().unwrap());
	assert_eq!(snapshot["tree"], reviewed["tree"]);
	let manifest_path = PathBuf::from(snapshot["manifest"].as_str().unwrap());
	let manifest: Value = serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
	assert_eq!(manifest["review"]["tree"], reviewed["tree"]);
	assert_eq!(
		git(&checkout, &["rev-parse", "HEAD^{tree}"]),
		snapshot["tree"]
	);
	assert_eq!(git(&checkout, &["show", "HEAD:file.txt"]), "candidate");

	let refs = git(&root, &["show-ref"]);
	let wrong_owner = b.call(
		"gitfs_snapshot",
		json!({
			"kind": "candidate", "id": snapshot["id"], "capability": snapshot["capability"]
		}),
	);
	rejects(
		&wrong_owner,
		"candidate snapshot belongs to a different session",
	);
	let wrong_capability = a.call(
		"gitfs_snapshot",
		json!({
			"kind": "candidate", "id": snapshot["id"], "capability": "f".repeat(64)
		}),
	);
	rejects(
		&wrong_capability,
		"candidate manifest ownership or repository mismatch",
	);
	let unknown = a.call(
		"gitfs_snapshot",
		json!({"kind": "candidate", "unexpected": true}),
	);
	rejects(&unknown, "unexpected is not valid for gitfs snapshot");
	let wrong_type = a.call("gitfs_snapshot", json!({"kind": true}));
	rejects(&wrong_type, "invalid type for kind");
	let mismatched_review = a.call(
		"gitfs_snapshot",
		json!({"kind": "candidate", "expected_review": {"tree": "not the preview"}}),
	);
	rejects(&mismatched_review, "candidate review changed since preview");
	assert_eq!(git(&root, &["show-ref"]), refs);
	assert!(checkout.exists());

	a.ok(
		"edit",
		json!({"path": "file.txt", "old": "candidate", "new": "later"}),
	);
	let stale = a.call(
		"gitfs_snapshot",
		json!({
			"kind": "candidate", "expected_tree": reviewed["tree"]
		}),
	);
	rejects(&stale, "candidate tree changed since preview");
	assert_eq!(git(&checkout, &["show", "HEAD:file.txt"]), "candidate");

	a.ok(
		"gitfs_snapshot",
		json!({
			"kind": "candidate", "id": snapshot["id"], "capability": snapshot["capability"]
		}),
	);
	assert!(!checkout.exists());
	assert!(!manifest_path.exists());
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), before_head);
	assert_eq!(
		std::fs::read(root.join(".git/index")).unwrap(),
		before_index
	);
	assert_eq!(
		std::fs::read_to_string(root.join("file.txt")).unwrap(),
		"before\n"
	);
}
