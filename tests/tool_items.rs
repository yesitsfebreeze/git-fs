//! Agent-neutral end-to-end coverage through a persistent stdio MCP connection.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Server {
	child: Child,
	input: ChildStdin,
	output: BufReader<ChildStdout>,
	next_id: u64,
}

impl Server {
	fn new(root: &Path) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_git-fs"))
			.arg("mcp")
			.current_dir(root)
			.env("GIT_FS_DISK", root)
			.env("GIT_FS_SESSION", "items-test")
			.env("GIT_FS_CONFIG", "{}")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.unwrap();
		let input = child.stdin.take().unwrap();
		let output = BufReader::new(child.stdout.take().unwrap());
		let mut server = Self {
			child,
			input,
			output,
			next_id: 0,
		};
		server.request("initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}));
		writeln!(
			server.input,
			"{}",
			json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
		)
		.unwrap();
		server
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		self.next_id += 1;
		writeln!(
			self.input,
			"{}",
			json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
		)
		.unwrap();
		self.input.flush().unwrap();
		let mut line = String::new();
		assert!(self.output.read_line(&mut line).unwrap() > 0);
		let reply: Value = serde_json::from_str(&line).unwrap();
		assert_eq!(reply["id"], self.next_id);
		assert!(reply.get("error").is_none(), "{reply}");
		reply["result"].clone()
	}

	fn call(&mut self, tool: &str, input: Value) -> Value {
		self.request("tools/call", json!({"name": tool, "arguments": input}))
	}

	fn ok(&mut self, tool: &str, input: Value) -> Value {
		let result = self.call(tool, input);
		assert_eq!(result["isError"], false, "{tool}: {result}");
		result
	}
}

impl Drop for Server {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn content(reply: &Value) -> Value {
	serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn git(root: &Path, args: &[&str]) -> String {
	let result = Command::new("git")
		.args(args)
		.current_dir(root)
		.output()
		.unwrap();
	assert!(
		result.status.success(),
		"{}",
		String::from_utf8_lossy(&result.stderr)
	);
	String::from_utf8(result.stdout).unwrap().trim().into()
}

fn repo() -> tempfile::TempDir {
	let dir = tempfile::tempdir().unwrap();
	git(dir.path(), &["init", "-q"]);
	git(dir.path(), &["config", "user.name", "Test"]);
	git(dir.path(), &["config", "user.email", "test@example.com"]);
	git(dir.path(), &["config", "commit.gpgsign", "false"]);
	for file in ["a.txt", "b.txt", "c.txt"] {
		std::fs::write(dir.path().join(file), "before\n").unwrap();
	}
	git(dir.path(), &["add", "."]);
	git(dir.path(), &["commit", "-qm", "base"]);
	dir
}

#[test]
fn single_calls_and_multiple_reads_edits_and_materializations_share_freshness() {
	let dir = repo();
	let root = dir.path().canonicalize().unwrap();
	let mut server = Server::new(&root);
	let tools = server.request("tools/list", json!({}));
	let tools = tools["tools"].as_array().unwrap();
	assert_eq!(tools.len(), 16);
	assert!(!tools.iter().any(|t| t["name"] == "batch"));
	for tool in tools {
		assert!(
			tool["inputSchema"]["properties"]["items"].is_object(),
			"{tool}"
		);
	}
	let single = server.ok("read", json!({"path": "c.txt"}));
	assert!(single["content"][0]["text"]
		.as_str()
		.unwrap()
		.contains("before"));
	let reads = content(&server.call(
		"read",
		json!({"items": [
		{"path": "a.txt"}, {"path": "b.txt"}, {"path": "missing"}
	], "concurrency": 2}),
	));
	assert_eq!(reads["failed"], 1);
	assert_eq!(reads["skipped"], 0);
	for index in 0..2 {
		assert_eq!(reads["results"][index]["index"], index);
		assert_eq!(reads["results"][index]["result"]["error"], false);
		assert!(reads["results"][index]["result"]["revision"].is_object());
	}
	let edits = content(&server.ok(
		"edit",
		json!({"items": [
			{"path": "a.txt", "old": "before", "new": "middle"},
			{"path": "a.txt", "old": "middle", "new": "after"},
			{"path": "b.txt", "edits": [{"old": "before", "new": "after"}]}
		]}),
	));
	assert_eq!(edits["results"].as_array().unwrap().len(), 3);
	assert_eq!(
		std::fs::read_to_string(root.join("a.txt")).unwrap(),
		"before\n"
	);
	server.ok(
		"gitfs_materialize",
		json!({"items": [{"paths": ["a.txt"]}, {"paths": ["b.txt"]}]}),
	);
	for file in ["a.txt", "b.txt"] {
		assert_eq!(std::fs::read_to_string(root.join(file)).unwrap(), "after\n");
	}
}

#[test]
fn mutations_stop_on_stale_read_and_report_remaining_items_without_rollback() {
	let dir = repo();
	let root = dir.path().canonicalize().unwrap();
	let mut server = Server::new(&root);
	server.ok(
		"read",
		json!({"items": [{"path": "a.txt"}, {"path": "b.txt"}, {"path": "c.txt"}]}),
	);
	std::fs::write(root.join("b.txt"), "external\n").unwrap();
	let reply = server.call(
		"write",
		json!({"items": [
			{"path": "a.txt", "text": "first\n"},
			{"path": "b.txt", "text": "must refuse\n"},
			{"path": "c.txt", "text": "must skip\n"}
		]}),
	);
	assert_eq!(reply["isError"], true);
	let result = content(&reply);
	assert_eq!(result["failed"], 1);
	assert_eq!(result["skipped"], 1);
	assert_eq!(result["results"][0]["result"]["error"], false);
	assert_eq!(result["results"][1]["result"]["error"], true);
	assert_eq!(result["results"][2]["skipped"], true);
	let reads = content(&server.ok(
		"read",
		json!({"items": [
			{"path": "a.txt", "raw": true}, {"path": "b.txt", "raw": true}, {"path": "c.txt", "raw": true}
		]}),
	));
	assert_eq!(reads["results"][0]["result"]["content"], "first\n");
	// A committed file without an owned overlay is read from trunk.
	assert_eq!(
		std::fs::read_to_string(root.join("b.txt")).unwrap(),
		"external\n"
	);
	assert_eq!(reads["results"][2]["result"]["content"], "before\n");
}

#[test]
fn invalid_envelopes_and_removed_batch_tool_execute_nothing() {
	let dir = repo();
	let root = dir.path().canonicalize().unwrap();
	let mut server = Server::new(&root);
	let head = git(&root, &["rev-parse", "HEAD"]);
	for input in [
		json!({"items": [{"path": "new.txt", "text": "bad"}, {"items": []}]}),
		json!({"items": [{"path": "new.txt", "text": "bad"}, {"_git_fs": {"session": "other"}}]}),
		json!({"items": [{"path": "new.txt", "text": "bad"}], "concurrency": 2}),
		json!({"items": [{"path": "new.txt", "text": "bad"}], "path": "a.txt"}),
	] {
		assert_eq!(server.call("write", input)["isError"], true);
		assert!(!root.join("new.txt").exists());
		assert!(git(&root, &["for-each-ref", "refs/gitfs/"]).is_empty());
	}
	assert_eq!(
		server.call("batch", json!({"requests": []}))["isError"],
		true
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), head);
}

#[test]
fn integration_items_preserve_preview_and_commit_review_guards() {
	let dir = repo();
	let root = dir.path().canonicalize().unwrap();
	let mut server = Server::new(&root);
	server.ok("read", json!({"path": "a.txt"}));
	server.ok(
		"edit",
		json!({"items": [{"path": "a.txt", "old": "before", "new": "after"}]}),
	);
	let original = git(&root, &["rev-parse", "HEAD"]);
	let previews = content(&server.ok(
		"integrate",
		json!({"items": [
			{"phase": "preview", "message": "test items"},
			{"phase": "preview", "message": "test items"}
		]}),
	));
	let revision = previews["results"][0]["result"]["content"]["revision"]
		.as_str()
		.unwrap();
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), original);
	let rejected = server.call(
		"integrate",
		json!({"items": [
			{"phase": "commit", "expected_revision": "stale", "message": "test items"},
			{"phase": "commit", "expected_revision": revision, "message": "test items"}
		]}),
	);
	assert_eq!(rejected["isError"], true);
	assert_eq!(content(&rejected)["skipped"], 1);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), original);
	server.ok(
		"integrate",
		json!({"items": [
			{"phase": "commit", "expected_revision": revision, "message": "test items"}
		]}),
	);
	assert_ne!(git(&root, &["rev-parse", "HEAD"]), original);
}
