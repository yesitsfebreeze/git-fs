//! The stdio MCP server: JSON-RPC 2.0, one JSON object per line (the
//! Content-Length framing older clients send is accepted too). Standard
//! clients bind once on their first tool call; Codex uses a separate stable
//! workspace binding for each session supplied by its trusted host hook.

use crate::service::Context;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const PROTOCOL: &str = "2025-06-18";

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

/// The session at this instant. A late `session-start` hook can select the
/// initial binding after MCP initialize, but changing it after the first tool
/// call is rejected by `Bindings`. The checkout-global CURRENT remains
/// compatibility-only; coordinated hosts must set GIT_FS_SESSION or supply a
/// distinct ancestry pin.
pub fn session() -> Result<String, String> {
	resolved_session().map(|(session, _)| session)
}

pub(crate) fn resolved_session() -> Result<(String, &'static str), String> {
	if let Ok(session) = std::env::var("GIT_FS_SESSION") {
		if !session.trim().is_empty() {
			return Ok((session, "environment"));
		}
	}
	let store = crate::store_config()?;
	// No repository and no store yet is not an error: the first write makes one.
	let Some(found) = crate::runtime_identity::read_existing_store(&store, &workspace()?)? else {
		return Ok(("main".into(), "fallback"));
	};
	let dir = found.root().join(store.directory());
	if let Some(session) = crate::session_pin::resolve(&dir) {
		return Ok((session, "session_pin"));
	}
	if let Ok(current) = std::fs::read_to_string(dir.join("CURRENT")) {
		let current = current.trim();
		if !current.is_empty() {
			return Ok((current.to_owned(), "current_file"));
		}
	}
	Ok(("main".into(), "fallback"))
}

pub fn context(run: &str, call: u64) -> Result<Context, String> {
	context_with_source(run, call).map(|(context, _)| context)
}

fn context_with_source(run: &str, call: u64) -> Result<(Context, &'static str), String> {
	let cwd = workspace()?;
	let (session, source) = resolved_session()?;
	let context = Context {
		session,
		run: format!("{run}-{}", std::process::id()),
		call: call.to_string(),
		cwd: cwd.to_string_lossy().into_owned(),
	};
	context.validate()?;
	Ok((context, source))
}

/// A workspace means the physical repository/store root, not the current
/// directory inside it. Keep the expected store location even before its
/// first write creates the directory, so symlink/relocation flips refuse.
#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundTarget {
	session: String,
	root: PathBuf,
	store_root: PathBuf,
	repository_mode: &'static str,
	source: &'static str,
}

impl BoundTarget {
	fn resolve(context: &Context, source: &'static str) -> Result<Self, String> {
		context.validate()?;
		let cwd = Path::new(&context.cwd)
			.canonicalize()
			.map_err(|e| format!("binding: workspace cannot be resolved: {e}"))?;
		let config = crate::store_config()?;
		let store = crate::runtime_identity::read_existing_store(&config, &cwd)?;
		let root = store
			.as_ref()
			.map(|s| s.root())
			.unwrap_or(&cwd)
			.canonicalize()
			.map_err(|e| format!("binding: repository root cannot be resolved: {e}"))?;
		let location = root.join(config.directory());
		let store_root = if location.exists() {
			location
				.canonicalize()
				.map_err(|e| format!("binding: store cannot be resolved: {e}"))?
		} else {
			location
		};
		let repository_mode = if root.join(".git").exists() {
			"git"
		} else if store.is_some() {
			"standalone"
		} else {
			"uninitialized"
		};
		Ok(Self {
			session: context.session.clone(),
			root,
			store_root,
			repository_mode,
			source,
		})
	}

	fn reconcile(&mut self, observed: &Self) -> Result<(), String> {
		if self.session != observed.session
			|| self.root != observed.root
			|| self.store_root != observed.store_root
			|| self.source != observed.source
		{
			return Err("binding mismatch: session, workspace, store or routing source changed; reconnect with a distinct session".into());
		}
		if self.repository_mode == "uninitialized" && observed.repository_mode == "standalone" {
			// A first mutation may create the previously absent standalone
			// store. This is the only expected repository-mode transition.
			self.repository_mode = "standalone";
		} else if self.repository_mode != observed.repository_mode {
			return Err(
				"binding mismatch: repository mode changed; reconnect before editing".into(),
			);
		}
		Ok(())
	}
}

/// One standard stdio process serves one caller. The Codex host can route
/// multiple real clients through one process; each routed session receives
/// its own immutable target and cannot borrow another client's workspace.
/// This is a drift guard, not peer authentication: a hub that multiplexes
/// unrelated clients onto one standard stdio pipe must create isolated
/// backends (or supply an authenticated caller identity). In coordinated
/// mode GIT_FS_COORDINATED=1 refuses the global CURRENT/fallback, but cannot
/// prove that a process pin belongs to a unique remote client.
#[derive(Default)]
struct Bindings {
	standard: Option<BoundTarget>,
	codex: HashMap<String, BoundTarget>,
}

impl Bindings {
	fn check(
		&mut self,
		context: &Context,
		codex: bool,
		source: &'static str,
	) -> Result<(), String> {
		if !codex
			&& matches!(
				std::env::var("GIT_FS_COORDINATED").as_deref(),
				Ok("1" | "true")
			) && !matches!(source, "environment" | "session_pin")
		{
			return Err("binding: coordinated MCP requires GIT_FS_SESSION or a unique session-start process pin; checkout-global CURRENT is not a client identity".into());
		}
		let observed = BoundTarget::resolve(context, source)?;
		let pinned = if codex {
			self.codex
				.entry(context.session.clone())
				.or_insert_with(|| observed.clone())
		} else {
			self.standard.get_or_insert_with(|| observed.clone())
		};
		pinned.reconcile(&observed)
	}
}

pub(crate) fn tools(codex: bool) -> Vec<Value> {
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

async fn dispatch(
	message: Value,
	calls: &AtomicU64,
	bindings: &mut Bindings,
	codex: bool,
) -> Option<Value> {
	let id = message.get("id").cloned();
	let method = message["method"].as_str().unwrap_or_default().to_owned();
	let params = message.get("params").cloned().unwrap_or(Value::Null);
	let reply = |result: Value| Some(json!({"jsonrpc": "2.0", "id": id, "result": result}));
	let fail = |code: i64, text: String| {
		Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": text}}))
	};
	match method.as_str() {
		"initialize" => {
			if !params.is_object()
				|| params["protocolVersion"]
					.as_str()
					.is_none_or(|revision| revision.trim().is_empty())
				|| !params["capabilities"].is_object()
				|| params["clientInfo"]["name"]
					.as_str()
					.is_none_or(|name| name.trim().is_empty())
				|| params["clientInfo"]["version"]
					.as_str()
					.is_none_or(|version| version.trim().is_empty())
			{
				return fail(-32602, "initialize requires protocolVersion, capabilities, and clientInfo name/version".into());
			}
			match crate::runtime_identity::inspect(codex) {
				Ok(mut identity) => {
					// The target observed at initialize is not yet a pinned
					// connection. SessionStart may still arrive before the first
					// tools/call. Codex binds independently for every routed session.
					identity["connection_binding_state"] = json!(if codex {
						"per_session"
					} else if bindings.standard.is_some() {
						"bound"
					} else {
						"pending"
					});
					reply(json!({
						"protocolVersion": PROTOCOL,
						"capabilities": {"tools": {}},
						"serverInfo": {"name": "git-fs", "version": env!("CARGO_PKG_VERSION")},
						"_meta": {"gitfs/runtimeIdentity": identity},
					}))
				}
				Err(error) => fail(-32603, error),
			}
		}
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
				crate::codex::take_context(&mut input, n).map(|context| (context, "codex_hook"))
			} else {
				context_with_source("mcp", n)
			};
			let answer = match call_context {
				Ok((context, source)) => match bindings.check(&context, codex, source) {
					Ok(()) => crate::call(&name, input, &context).await,
					Err(e) => Err(e),
				},
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
/// before the edit is judged. Read/search tools parallelize their own `items`
/// internally; mutations and other tool calls still serialize here.
pub fn serve(codex: bool) -> i32 {
	let runtime = runtime();
	if let Err(e) = crate::env_config().and_then(crate::start) {
		eprintln!("git-fs: {e}");
		return 1;
	}
	let calls = std::sync::Arc::new(AtomicU64::new(0));
	let mut bindings = Bindings::default();
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
		if let Some(reply) = runtime.block_on(dispatch(message, &calls, &mut bindings, codex)) {
			let _ = tx.send(reply);
		}
	}
	drop(tx);
	let _ = writer.join();
	0
}
