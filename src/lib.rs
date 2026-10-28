//! git-fs: workspace file access and the Git session branch it commits to.
//! In `git` mode every write and edit lands as one commit on
//! `refs/gitfs/<session>` and the worktree stays untouched until materialize;
//! reads and searches prefer the session's changes and fall back to the
//! worktree. Integrate reviews that branch onto trunk; `local` mode writes
//! directly. One process serves it all: the stdio MCP server (`git-fs mcp`),
//! the Claude Code lifecycle hooks (`git-fs hook <action>`), a one-shot CLI
//! (`git-fs call <tool> <json>`), and the private pre-push hook.

mod batch;
pub mod bridge;
mod change_record;
pub mod codex;
mod config;
mod files;
pub mod hook;
pub mod hooks;
mod integrate;
mod limits;
pub mod mcp;
pub mod overlay;
mod provenance;
mod push;
pub mod review;
mod search;
mod secrets;
pub mod service;
pub mod store;
mod tool_result;

pub use service::document;

use provenance::Recorder;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

pub(crate) type Result<T, E = String> = std::result::Result<T, E>;

/// Every tool the MCP server lists, by the name an agent calls it.
pub const TOOLS: [&str; 16] = [
	"read",
	"write",
	"edit",
	"glob",
	"grep",
	"search",
	"gitfs_ls",
	"gitfs_diff",
	"gitfs_rollback",
	"gitfs_catchup",
	"gitfs_materialize",
	"gitfs_capture",
	"integrate",
	"push",
	"undo",
	"scan",
];

struct Fs {
	service: Arc<service::Service>,
	integrate: integrate::Config,
	recorder: Recorder,
}

static FS: OnceLock<Fs> = OnceLock::new();

fn fs() -> Result<&'static Fs> {
	FS.get().ok_or_else(|| "git-fs not configured".to_owned())
}

/// The executable a push links as the repository's private pre-push hook:
/// this program itself, which answers as the hook when git invokes it under
/// that name (see `hook::hook_entry`).
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn hook_program() -> Result<PathBuf> {
	std::env::current_exe().map_err(|e| format!("own executable unknown: {e}"))
}

/// Settle the configuration and build the service, once per process.
/// `config` is laid over the defaults `manifest.md` declares.
pub fn start(config: Value) -> Result<()> {
	let config = service::Config::parse(config)?;
	limits::install(config.limits);
	let integrate_settings = config.integrate.clone();
	// No `sessions` kern to attribute mutations to: the touch is a no-op.
	let touch: service::Touch = Arc::new(|_, _| Box::pin(async { Ok(()) }));
	let service = Arc::new(service::Service::new(config, touch));
	let fs = Fs {
		integrate: integrate::Config::from(&integrate_settings, service.store.clone()),
		recorder: Recorder::Absent,
		service,
	};
	FS.set(fs)
		.map_err(|_| "git-fs already configured".to_string())
}

/// Configuration from the environment: `GIT_FS_CONFIG` holds a JSON object
/// laid over the declared defaults; nothing set means the defaults.
pub fn env_config() -> Result<Value> {
	match std::env::var("GIT_FS_CONFIG") {
		Ok(text) if !text.trim().is_empty() => {
			serde_json::from_str(&text).map_err(|e| format!("GIT_FS_CONFIG: {e}"))
		}
		_ => Ok(json!({})),
	}
}

/// The MCP-facing description of one tool: name, description, input schema.
pub fn describe(tool: &str) -> Result<Value> {
	Ok(batch::describe(tool, describe_single(tool)?))
}

fn describe_single(tool: &str) -> Result<Value> {
	if let Some((family, operation)) = dotted(tool) {
		let event = format!("tool.{family}.{operation}");
		let document = manifest();
		let declaration = &document["events"][&event];
		if declaration.is_null() {
			return Err(format!("unknown tool {tool}"));
		}
		return Ok(json!({
			"name": tool,
			"description": declaration["description"],
			"input_schema": declaration["schema"]["properties"]["input"],
		}));
	}
	if !TOOLS.contains(&tool) {
		return Err(format!("unknown tool {tool}"));
	}
	Ok(service::describe(tool))
}

fn manifest() -> &'static Value {
	static DOCUMENT: OnceLock<Value> = OnceLock::new();
	DOCUMENT.get_or_init(|| serde_json::from_str(document()).expect("git-fs manifest"))
}

/// The gitfs and integrate operations, by the tool name they answer to.
fn dotted(tool: &str) -> Option<(&'static str, &'static str)> {
	Some(match tool {
		"gitfs_ls" => ("gitfs", "ls"),
		"gitfs_diff" => ("gitfs", "diff"),
		"gitfs_rollback" => ("gitfs", "rollback"),
		"gitfs_catchup" => ("gitfs", "catchup"),
		"gitfs_materialize" => ("gitfs", "materialize"),
		"gitfs_capture" => ("gitfs", "capture"),
		"integrate" => ("integrate", "integrate"),
		"push" => ("integrate", "push"),
		"undo" => ("integrate", "undo"),
		"scan" => ("integrate", "scan"),
		_ => return None,
	})
}

/// One invocation: the tool's input under the session, run and call the
/// caller names, in the workspace `cwd`. Answers `{content, error}`, with a
/// `revision` on reads and writes.
pub async fn call(tool: &str, input: Value, context: &service::Context) -> Result<Value> {
	if !TOOLS.contains(&tool) {
		return Err(format!("unknown tool {tool}"));
	}
	if input.get("items").is_some() {
		return batch::run(tool, input, context).await;
	}
	call_single(tool, input, context).await
}

async fn call_single(tool: &str, input: Value, context: &service::Context) -> Result<Value> {
	let fs = fs()?;
	let envelope = json!({"op": "call", "input": input, "context": context});
	if let Some((family, operation)) = dotted(tool) {
		let args = operation_request(family, operation, envelope)?;
		return Ok(match family {
			"gitfs" => {
				overlay::handle_with_recorder(fs.service.store.clone(), args, fs.recorder.clone())
					.await
			}
			_ => integrate::handle(bridge::Ctx, &fs.integrate, args).await,
		}
		.unwrap_or_else(tool_result::failure));
	}
	let name: &'static str = TOOLS
		.iter()
		.copied()
		.find(|t| *t == tool)
		.ok_or_else(|| format!("unknown tool {tool}"))?;
	Ok(fs.service.dispatch(name, envelope).await)
}

/// Cancel one invocation by its context.
pub async fn cancel(tool: &str, context: &service::Context) -> Result<Value> {
	let fs = fs()?;
	let envelope = json!({"op": "cancel", "context": context});
	if let Some((family, _)) = dotted(tool) {
		return Ok(match family {
			"gitfs" => {
				overlay::handle_with_recorder(
					fs.service.store.clone(),
					envelope,
					fs.recorder.clone(),
				)
				.await
			}
			_ => integrate::handle(bridge::Ctx, &fs.integrate, envelope).await,
		}
		.unwrap_or_else(tool_result::failure));
	}
	let name: &'static str = TOOLS
		.iter()
		.copied()
		.find(|t| *t == tool)
		.ok_or_else(|| format!("unknown tool {tool}"))?;
	Ok(fs.service.dispatch(name, envelope).await)
}

fn operation_request(family: &str, operation: &str, mut args: Value) -> Result<Value> {
	let input = args["input"]
		.as_object_mut()
		.ok_or("tool input must be an object")?;
	if input.contains_key("op") {
		return Err("the operation is selected by the tool name".into());
	}
	input.insert("op".into(), Value::String(operation.into()));
	tool_result::validate(family, &args["input"])?;
	Ok(args)
}

/// The store the hooks and CLI reach without the service: the configured
/// store directory, resolved at a workspace.
pub fn store_config() -> Result<Arc<store::StoreConfig>> {
	Ok(fs()?.service.store.clone())
}
