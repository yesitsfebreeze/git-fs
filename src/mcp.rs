//! The stdio MCP server: JSON-RPC 2.0, one JSON object per line (the
//! Content-Length framing older clients send is accepted too). Every tool
//! runs under one session: `GIT_FS_SESSION`, else the session the
//! `session-start` hook pinned in `<store>/CURRENT`, else `main`.

use crate::service::Context;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};

const PROTOCOL: &str = "2025-06-18";

pub fn runtime() -> tokio::runtime::Runtime {
	tokio::runtime::Builder::new_multi_thread()
		.enable_all()
		.build()
		.expect("tokio runtime")
}

/// The workspace the tools answer for: `GIT_FS_DISK`, else the directory the
/// server was launched in, which is how Claude Code launches MCP servers.
pub fn workspace() -> Result<std::path::PathBuf, String> {
	match std::env::var_os("GIT_FS_DISK") {
		Some(dir) if !dir.is_empty() => Ok(std::path::PathBuf::from(dir)),
		_ => std::env::current_dir().map_err(|e| format!("workspace unavailable: {e}")),
	}
}

/// The session every call runs as. Read on each call, so a `session-start`
/// hook that fires after the server came up still wins. Ceiling: two Claude
/// sessions in one repository share `CURRENT`, and the later one wins both;
/// set `GIT_FS_SESSION` per server to keep them apart.
pub fn session() -> Result<String, String> {
	if let Ok(session) = std::env::var("GIT_FS_SESSION") {
		if !session.trim().is_empty() {
			return Ok(session);
		}
	}
	let store = crate::store_config()?;
	if let Ok(current) = std::fs::read_to_string(
		store
			.at_readonly(&workspace()?)
			.map(|s| s.root().join(store.directory()).join("CURRENT"))?,
	) {
		let current = current.trim();
		if !current.is_empty() {
			return Ok(current.to_owned());
		}
	}
	Ok("main".into())
}

pub fn context(run: &str, call: u64) -> Result<Context, String> {
	let cwd = workspace()?;
	let context = Context {
		session: session()?,
		run: format!("{run}-{}", std::process::id()),
		call: call.to_string(),
		cwd: cwd.to_string_lossy().into_owned(),
	};
	context.validate()?;
	Ok(context)
}

fn tools(codex: bool) -> Vec<Value> {
	crate::TOOLS
		.iter()
		.filter_map(|tool| crate::describe(tool).ok())
		.map(|mut d| {
			if codex {
				crate::codex::describe_context(&mut d["input_schema"]);
			}
			json!({
				"name": d["name"],
				"description": d["description"],
				"inputSchema": d["input_schema"],
			})
		})
		.collect()
}

/// A tool answer `{content, error, revision?}` as an MCP tool result.
fn tool_result(answer: Value) -> Value {
	let is_error = answer["error"] == true;
	let mut text = match &answer["content"] {
		Value::String(s) => s.clone(),
		Value::Null => String::new(),
		other => serde_json::to_string_pretty(other).unwrap_or_default(),
	};
	if let Some(revision) = answer.get("revision").filter(|r| !r.is_null()) {
		text.push_str("\n\nrevision: ");
		text.push_str(&serde_json::to_string(revision).unwrap_or_default());
	}
	json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

async fn dispatch(message: Value, calls: &AtomicU64, codex: bool) -> Option<Value> {
	let id = message.get("id").cloned();
	let method = message["method"].as_str().unwrap_or_default().to_owned();
	let params = message.get("params").cloned().unwrap_or(Value::Null);
	let reply = |result: Value| Some(json!({"jsonrpc": "2.0", "id": id, "result": result}));
	let fail = |code: i64, text: String| {
		Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": text}}))
	};
	match method.as_str() {
		"initialize" => reply(json!({
			"protocolVersion": params["protocolVersion"].as_str().unwrap_or(PROTOCOL),
			"capabilities": {"tools": {}},
			"serverInfo": {"name": "git-fs", "version": env!("CARGO_PKG_VERSION")},
		})),
		"ping" => reply(json!({})),
		"tools/list" => reply(json!({"tools": tools(codex)})),
		"tools/call" => {
			let name = params["name"].as_str().unwrap_or_default().to_owned();
			let mut input = match params.get("arguments").cloned() {
				Some(Value::Object(map)) => Value::Object(map),
				_ => json!({}),
			};
			let n = calls.fetch_add(1, Ordering::Relaxed) + 1;
			let call_context = if codex {
				crate::codex::take_context(&mut input, n)
			} else {
				context("mcp", n)
			};
			let answer = match call_context {
				Ok(context) => crate::call(&name, input, &context).await,
				Err(e) => Err(e),
			};
			match answer {
				Ok(value) => reply(tool_result(value)),
				Err(e) => reply(json!({"content": [{"type": "text", "text": e}], "isError": true})),
			}
		}
		_ if id.is_none() => None,
		other => fail(-32601, format!("method not found: {other}")),
	}
}

/// Serve until stdin closes. Requests run one at a time, in arrival order:
/// a client that sends a read and then an edit gets the read registered
/// before the edit is judged, and parallel tool calls serialize here.
pub fn serve(codex: bool) -> i32 {
	let runtime = runtime();
	if let Err(e) = crate::env_config().and_then(crate::start) {
		eprintln!("git-fs: {e}");
		return 1;
	}
	let calls = std::sync::Arc::new(AtomicU64::new(0));
	let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
	let writer = std::thread::spawn(move || {
		let mut out = std::io::stdout().lock();
		while let Some(value) = rx.blocking_recv() {
			let line = serde_json::to_string(&value).unwrap_or_default();
			if out
				.write_all(line.as_bytes())
				.and_then(|()| out.write_all(b"\n"))
				.and_then(|()| out.flush())
				.is_err()
			{
				break;
			}
		}
	});
	let stdin = std::io::stdin();
	let mut lines = stdin.lock();
	let mut buffer = String::new();
	loop {
		buffer.clear();
		match lines.read_line(&mut buffer) {
			Ok(0) | Err(_) => break,
			Ok(_) => {}
		}
		let line = buffer.trim();
		if line.is_empty() {
			continue;
		}
		// Content-Length framing: the header, a blank line, then the body.
		let body = if let Some(rest) = line.strip_prefix("Content-Length:") {
			let Ok(len) = rest.trim().parse::<usize>() else {
				continue;
			};
			// The blank line between the header and the body.
			let mut skip = String::new();
			if lines.read_line(&mut skip).is_err() {
				break;
			}
			let mut bytes = vec![0; len];
			if std::io::Read::read_exact(&mut lines, &mut bytes).is_err() {
				break;
			}
			String::from_utf8_lossy(&bytes).into_owned()
		} else {
			line.to_owned()
		};
		let Ok(message) = serde_json::from_str::<Value>(&body) else {
			continue;
		};
		if let Some(reply) = runtime.block_on(dispatch(message, &calls, codex)) {
			let _ = tx.send(reply);
		}
	}
	drop(tx);
	let _ = writer.join();
	0
}
