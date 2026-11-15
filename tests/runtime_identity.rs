//! Runtime identity and MCP negotiation observed through the compiled executable.
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn exchange(root: &Path, params: &[Value]) -> Vec<Value> {
	exchange_profile(root, params, false)
}

fn exchange_profile(root: &Path, params: &[Value], codex: bool) -> Vec<Value> {
	let mut command = Command::new(env!("CARGO_BIN_EXE_git-fs"));
	command.arg("mcp");
	if codex {
		command.arg("codex");
	}
	let mut child = command
		.current_dir(root)
		.env("GIT_FS_DISK", root)
		.env("GIT_FS_SESSION", "identity-test")
		.env("GIT_FS_CONFIG", "{}")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();
	{
		let mut input = child.stdin.take().unwrap();
		for (index, request) in params.iter().enumerate() {
			writeln!(input, "{}", json!({"jsonrpc":"2.0", "id": index + 1, "method": request["method"], "params": request["params"]})).unwrap();
		}
	}
	let output = child.wait_with_output().unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	let replies = String::from_utf8(output.stdout).unwrap();
	replies
		.lines()
		.map(|line| serde_json::from_str(line).unwrap())
		.collect()
}

fn initialize(protocol: Value) -> Value {
	json!({"method":"initialize", "params":{"protocolVersion":protocol,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}})
}

#[test]
fn initialize_negotiates_only_implemented_revisions() {
	let dir = tempfile::tempdir().unwrap();
	for revision in ["2025-06-18", "2099-01-01"] {
		let reply = exchange(dir.path(), &[initialize(json!(revision))]).remove(0);
		assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
		let runtime = &reply["result"]["_meta"]["gitfs/runtimeIdentity"];
		assert_eq!(runtime["schema_version"], 1);
		assert_eq!(runtime["binding_state"], "bound");
		assert_eq!(runtime["tool_schema_profile"], "standard");
		assert_eq!(
			runtime["supported_protocol_versions"],
			json!(["2025-06-18"])
		);
		assert!(runtime["tool_schema_fingerprint"].as_str().is_some());
		assert_eq!(
			runtime["build"]["package_version"],
			env!("CARGO_PKG_VERSION")
		);
		assert_eq!(runtime["build"]["revision"], Value::Null);
	}
	assert!(
		!dir.path().join(".git-fs").exists(),
		"initialize must be read-only"
	);
}

#[test]
fn malformed_initialization_params_are_rejected() {
	let dir = tempfile::tempdir().unwrap();
	let requests = [
		json!({"method":"initialize", "params":{}}),
		json!({"method":"initialize", "params":{"protocolVersion":4}}),
		json!({"method":"initialize", "params":{"protocolVersion":""}}),
		json!({"method":"initialize", "params":null}),
		json!({"method":"initialize", "params":{"protocolVersion":"2025-06-18", "clientInfo":{"name":"test","version":"1"}}}),
		json!({"method":"initialize", "params":{"protocolVersion":"2025-06-18", "capabilities":[], "clientInfo":{"name":"test","version":"1"}}}),
		json!({"method":"initialize", "params":{"protocolVersion":"2025-06-18", "capabilities":{}, "clientInfo":{"name":"test"}}}),
	];
	for request in requests {
		let reply = exchange(dir.path(), &[request]).remove(0);
		assert_eq!(reply["error"]["code"], -32602, "{reply}");
	}
	assert!(!dir.path().join(".git-fs").exists());
}

#[test]
fn identity_inspection_tracks_root_and_session_without_creating_a_store() {
	let dir = tempfile::tempdir().unwrap();
	let nested = dir.path().join("nested");
	std::fs::create_dir(&nested).unwrap();
	let replies = exchange(&nested, &[initialize(json!("2025-06-18"))]);
	let identity = &replies[0]["result"]["_meta"]["gitfs/runtimeIdentity"];
	assert_eq!(identity["session"]["id"], "identity-test");
	assert_eq!(identity["session"]["source"], "environment");
	assert_eq!(identity["repository_mode"], "uninitialized");
	let canonical_nested = nested
		.canonicalize()
		.unwrap()
		.to_string_lossy()
		.replace('\\', "/");
	assert_eq!(
		identity["workspace_root"]
			.as_str()
			.unwrap()
			.to_ascii_lowercase(),
		canonical_nested.to_ascii_lowercase()
	);
	assert_eq!(identity["store_root"], Value::Null);
	assert!(!nested.join(".git-fs").exists());

	let status = Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap();
	assert!(status.success());
	let committed = Command::new("git")
		.args([
			"-c",
			"user.name=Runtime Test",
			"-c",
			"user.email=runtime@test.invalid",
			"commit",
			"--allow-empty",
			"-m",
			"baseline",
			"-q",
		])
		.current_dir(dir.path())
		.status()
		.unwrap();
	assert!(committed.success());
	let replies = exchange(&nested, &[initialize(json!("2025-06-18"))]);
	let identity = &replies[0]["result"]["_meta"]["gitfs/runtimeIdentity"];
	let canonical = dir
		.path()
		.canonicalize()
		.unwrap()
		.to_string_lossy()
		.replace('\\', "/");
	assert_eq!(
		identity["workspace_root"]
			.as_str()
			.unwrap()
			.to_ascii_lowercase(),
		canonical.to_ascii_lowercase()
	);
	assert_eq!(identity["repository_mode"], "git");
	let head = Command::new("git")
		.args(["rev-parse", "HEAD"])
		.current_dir(dir.path())
		.output()
		.unwrap();
	assert!(head.status.success());
	let head = String::from_utf8(head.stdout).unwrap();
	assert_eq!(identity["revisions"]["head"], head.trim());
	assert_eq!(
		identity["build"]["revision"],
		Value::Null,
		"binary build stamp cannot be inferred from checkout HEAD"
	);
	assert!(!dir.path().join(".git-fs").exists());
}
#[test]
fn cli_and_mcp_ls_expose_same_read_only_identity() {
	let dir = tempfile::tempdir().unwrap();
	let initialized = Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap();
	assert!(initialized.success());
	let cli = Command::new(env!("CARGO_BIN_EXE_git-fs"))
		.args(["call", "gitfs_ls"])
		.current_dir(dir.path())
		.env("GIT_FS_DISK", dir.path())
		.env("GIT_FS_SESSION", "identity-test")
		.env("GIT_FS_CONFIG", "{}")
		.output()
		.unwrap();
	assert!(
		cli.status.success(),
		"{}",
		String::from_utf8_lossy(&cli.stderr)
	);
	let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
	assert_eq!(cli["error"], false);
	let cli_identity = &cli["content"]["runtime"];
	assert_eq!(cli_identity["schema_version"], 1);
	assert_eq!(cli_identity["session"]["source"], "invocation_context");

	let response = exchange(
		dir.path(),
		&[
			initialize(json!("2025-06-18")),
			json!({"method":"tools/call", "params":{"name":"gitfs_ls", "arguments":{}}}),
		],
	);
	assert_eq!(
		response[0]["result"]["_meta"]["gitfs/runtimeIdentity"]["tool_schema_fingerprint"],
		cli_identity["tool_schema_fingerprint"]
	);
	let text = response[1]["result"]["content"][0]["text"]
		.as_str()
		.unwrap();
	let tool_answer: Value = serde_json::from_str(text).unwrap();
	assert_eq!(
		tool_answer["runtime"]["tool_schema_fingerprint"],
		cli_identity["tool_schema_fingerprint"]
	);
	let peer = Command::new(env!("CARGO_BIN_EXE_git-fs"))
		.args(["call", "gitfs_ls", r#"{"session":"peer"}"#])
		.current_dir(dir.path())
		.env("GIT_FS_DISK", dir.path())
		.env("GIT_FS_SESSION", "identity-test")
		.env("GIT_FS_CONFIG", "{}")
		.output()
		.unwrap();
	assert!(
		peer.status.success(),
		"{}",
		String::from_utf8_lossy(&peer.stderr)
	);
	let peer: Value = serde_json::from_slice(&peer.stdout).unwrap();
	assert_eq!(
		peer["content"]["session"], "peer",
		"top-level session is the inspected branch"
	);
	assert_eq!(
		peer["content"]["runtime"]["session"]["id"], "identity-test",
		"runtime identity remains the caller"
	);
	assert!(
		!dir.path().join(".git-fs").exists(),
		"inspection must not create a store"
	);
}

#[test]
fn codex_initialize_cannot_claim_unverified_cwd_or_session() {
	let dir = tempfile::tempdir().unwrap();
	let normal = exchange(dir.path(), &[initialize(json!("2025-06-18"))]);
	let codex = exchange_profile(dir.path(), &[initialize(json!("2025-06-18"))], true);
	let normal = &normal[0]["result"]["_meta"]["gitfs/runtimeIdentity"];
	let codex = &codex[0]["result"]["_meta"]["gitfs/runtimeIdentity"];
	assert_eq!(codex["binding_state"], "unbound");
	assert_eq!(codex["repository_mode"], "unknown");
	assert_eq!(codex["workspace_root"], Value::Null);
	assert_eq!(codex["store_root"], Value::Null);
	assert_eq!(codex["session"]["id"], Value::Null);
	assert_eq!(codex["session"]["source"], "unbound");
	assert_eq!(codex["revisions"]["head"], Value::Null);
	assert_eq!(codex["tool_schema_profile"], "codex");
	assert_ne!(
		codex["tool_schema_fingerprint"],
		normal["tool_schema_fingerprint"]
	);
	assert!(!dir.path().join(".git-fs").exists());
}

#[test]
fn existing_standalone_store_is_inspected_without_mutation() {
	let dir = tempfile::tempdir().unwrap();
	let store = gitfs::store::StoreConfig::new(".git-fs")
		.at(dir.path())
		.unwrap();
	drop(store);
	let git_dir = dir.path().join(".git-fs");
	let before_head = std::fs::read(git_dir.join("HEAD")).unwrap();
	let before_config = std::fs::read(git_dir.join("config")).unwrap();
	assert!(!git_dir.join("CURRENT").exists());

	let reply = exchange(dir.path(), &[initialize(json!("2025-06-18"))]).remove(0);
	let runtime = &reply["result"]["_meta"]["gitfs/runtimeIdentity"];
	assert_eq!(runtime["repository_mode"], "standalone");
	assert_eq!(runtime["binding_state"], "bound");
	assert_eq!(runtime["session"]["id"], "identity-test");
	let expected = git_dir
		.canonicalize()
		.unwrap()
		.to_string_lossy()
		.replace('\\', "/");
	assert_eq!(
		runtime["store_root"].as_str().unwrap().to_ascii_lowercase(),
		expected.to_ascii_lowercase()
	);
	assert_eq!(std::fs::read(git_dir.join("HEAD")).unwrap(), before_head);
	assert_eq!(
		std::fs::read(git_dir.join("config")).unwrap(),
		before_config
	);
	assert!(
		!git_dir.join("CURRENT").exists(),
		"inspection must not create a session pin"
	);
}
