//! Runtime binding must be stable across calls in a long-lived MCP server.
//! These tests exercise real stdio JSON-RPC clients, not only the resolver.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};

struct Client {
	child: Child,
	output: BufReader<ChildStdout>,
	next_id: usize,
}

impl Client {
	fn start(root: &Path, codex: bool, session: Option<&str>, coordinated: bool) -> Self {
		let mut command = Command::new(env!("CARGO_BIN_EXE_git-fs"));
		command.arg("mcp");
		if codex {
			command.arg("codex");
		}
		command
			.current_dir(root)
			.env("GIT_FS_DISK", root)
			.env("GIT_FS_CONFIG", "{}")
			.env_remove("GIT_FS_SESSION")
			.env_remove("GIT_FS_COORDINATED")
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped());
		if let Some(session) = session {
			command.env("GIT_FS_SESSION", session);
		}
		if coordinated {
			command.env("GIT_FS_COORDINATED", "1");
		}
		let mut child = command.spawn().unwrap();
		let output = BufReader::new(child.stdout.take().unwrap());
		Self {
			child,
			output,
			next_id: 0,
		}
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		self.next_id += 1;
		let request = json!({"jsonrpc":"2.0", "id":self.next_id, "method":method, "params":params});
		let input = self.child.stdin.as_mut().unwrap();
		writeln!(input, "{request}").unwrap();
		input.flush().unwrap();
		let mut line = String::new();
		assert_ne!(
			self.output.read_line(&mut line).unwrap(),
			0,
			"MCP server exited before replying"
		);
		serde_json::from_str(&line).unwrap()
	}

	fn init(&mut self) -> Value {
		self.request(
			"initialize",
			json!({
				"protocolVersion":"2025-06-18",
				"capabilities":{},
				"clientInfo":{"name":"binding-test","version":"1"},
			}),
		)
	}

	fn tool(&mut self, name: &str, arguments: Value) -> Value {
		self.request("tools/call", json!({"name":name,"arguments":arguments}))["result"].clone()
	}
}

impl Drop for Client {
	fn drop(&mut self) {
		self.child.stdin.take();
		let _ = self.child.kill();
		let _ = self.child.wait();
	}
}

fn repository() -> tempfile::TempDir {
	let dir = tempfile::tempdir().unwrap();
	assert!(Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	dir
}

fn current(root: &Path, session: &str) {
	let store = root.join(".git-fs");
	std::fs::create_dir_all(&store).unwrap();
	std::fs::write(store.join("CURRENT"), format!("{session}\n")).unwrap();
}

fn caller_session(result: &Value) -> String {
	assert_eq!(result["isError"], false, "{result}");
	let content = result["content"][0]["text"].as_str().unwrap();
	let value: Value = serde_json::from_str(content).unwrap();
	value["runtime"]["session"]["id"].as_str().unwrap().into()
}

fn assert_binding_mismatch(result: &Value) {
	assert_eq!(result["isError"], true, "{result}");
	let message = result["content"][0]["text"].as_str().unwrap();
	assert!(
		message.contains("binding") || message.contains("identity"),
		"{message}"
	);
}

#[test]
fn current_flip_after_first_call_is_rejected() {
	let repo = repository();
	current(repo.path(), "first");
	let mut client = Client::start(repo.path(), false, None, false);
	client.init();
	assert_eq!(caller_session(&client.tool("gitfs_ls", json!({}))), "first");
	current(repo.path(), "second");
	assert_binding_mismatch(&client.tool("gitfs_ls", json!({})));
	drop(client);
	let mut reconnected = Client::start(repo.path(), false, None, false);
	reconnected.init();
	assert_eq!(
		caller_session(&reconnected.tool("gitfs_ls", json!({}))),
		"second"
	);
}

#[test]
fn drift_rejects_capture_before_touching_any_refs_or_files() {
	let repo = repository();
	current(repo.path(), "first");
	let mut client = Client::start(repo.path(), false, None, false);
	client.init();
	assert_eq!(caller_session(&client.tool("gitfs_ls", json!({}))), "first");
	let file = repo.path().join("candidate.txt");
	std::fs::write(&file, b"unpublished\n").unwrap();
	current(repo.path(), "second");
	let refs = || {
		Command::new("git")
			.args(["show-ref"])
			.current_dir(repo.path())
			.output()
			.unwrap()
			.stdout
	};
	let before = refs();
	assert_binding_mismatch(&client.tool("gitfs_capture", json!({"paths":["candidate.txt"]})));
	assert_eq!(
		refs(),
		before,
		"a mismatched client cannot create or move any refs"
	);
	assert_eq!(std::fs::read(&file).unwrap(), b"unpublished\n");
}

#[test]
fn process_pin_flip_after_first_call_is_rejected() {
	let repo = repository();
	let mut client = Client::start(repo.path(), false, None, false);
	client.init();
	let sessions = repo.path().join(".git-fs/sessions");
	std::fs::create_dir_all(&sessions).unwrap();
	let pin = sessions.join(format!("{}.session", client.child.id()));
	std::fs::write(&pin, b"first\n").unwrap();
	assert_eq!(caller_session(&client.tool("gitfs_ls", json!({}))), "first");
	std::fs::write(&pin, b"second\n").unwrap();
	assert_binding_mismatch(&client.tool("gitfs_ls", json!({})));
}

#[test]
fn late_pin_before_first_call_wins_over_initialize() {
	let repo = repository();
	current(repo.path(), "old");
	let mut client = Client::start(repo.path(), false, None, false);
	let initialized = client.init();
	assert_eq!(
		initialized["result"]["_meta"]["gitfs/runtimeIdentity"]["session"]["id"],
		"old"
	);
	assert_eq!(
		initialized["result"]["_meta"]["gitfs/runtimeIdentity"]["connection_binding_state"],
		"pending"
	);
	let sessions = repo.path().join(".git-fs/sessions");
	std::fs::create_dir_all(&sessions).unwrap();
	std::fs::write(
		sessions.join(format!("{}.session", client.child.id())),
		b"late\n",
	)
	.unwrap();
	assert_eq!(caller_session(&client.tool("gitfs_ls", json!({}))), "late");
}

#[test]
fn separate_generic_clients_can_share_one_repository_without_identity_bleed() {
	let repo = repository();
	let mut a = Client::start(repo.path(), false, Some("agent-a"), false);
	let mut b = Client::start(repo.path(), false, Some("agent-b"), false);
	a.init();
	b.init();
	assert_eq!(caller_session(&a.tool("gitfs_ls", json!({}))), "agent-a");
	assert_eq!(caller_session(&b.tool("gitfs_ls", json!({}))), "agent-b");
	assert_eq!(caller_session(&a.tool("gitfs_ls", json!({}))), "agent-a");
}

#[test]
fn coordinated_clients_refuse_global_current_fallback() {
	let repo = repository();
	current(repo.path(), "global");
	let mut client = Client::start(repo.path(), false, None, true);
	client.init();
	let result = client.tool("gitfs_ls", json!({}));
	assert_binding_mismatch(&result);
	assert!(
		result["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("GIT_FS_SESSION"),
		"{result}"
	);
}

#[test]
fn codex_clients_bind_each_session_to_one_workspace() {
	let a = repository();
	let b = repository();
	let mut client = Client::start(a.path(), true, None, false);
	let init = client.init();
	assert_eq!(
		init["result"]["_meta"]["gitfs/runtimeIdentity"]["binding_state"],
		"unbound"
	);
	assert_eq!(
		init["result"]["_meta"]["gitfs/runtimeIdentity"]["connection_binding_state"],
		"per_session"
	);
	let route = |session: &str, root: &Path| json!({"_git_fs":{"session":session,"cwd":root}});
	assert_eq!(
		caller_session(&client.tool("gitfs_ls", route("first", a.path()))),
		"first"
	);
	assert_eq!(
		caller_session(&client.tool("gitfs_ls", route("second", b.path()))),
		"second"
	);
	let nested = a.path().join("nested");
	std::fs::create_dir(&nested).unwrap();
	assert_eq!(
		caller_session(&client.tool("gitfs_ls", route("first", &nested))),
		"first"
	);
	assert_eq!(
		caller_session(&client.tool("gitfs_ls", route("first", a.path()))),
		"first"
	);
	assert_binding_mismatch(&client.tool("gitfs_ls", route("first", b.path())));
	let missing = client.tool("gitfs_ls", json!({}));
	assert_eq!(missing["isError"], true);
	assert!(
		missing["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("routing"),
		"{missing}"
	); // Never guess from process cwd.
}
