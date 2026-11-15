//! Codex wire adapter. Trusted host hooks bind each MCP call to its originating
//! session/workspace instead of the process cwd or checkout-global CURRENT.
//! The MCP wire format itself cannot authenticate model-supplied `_git_fs`:
//! the host must inject/overwrite routing on every call and prevent bypass.
use crate::service::{Context, Mode};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

const CONTEXT: &str = "_git_fs";

pub fn guide(session: &str, owned: usize, mode: Mode) -> String {
	crate::hooks::guide(session, owned, mode)
		.replace("mcp__plugin_git-fs_git-fs__*", "mcp__git_fs__* (discover the git-fs server's tools if deferred)")
		.replace("Edit, Write or a heredoc", "apply_patch or a heredoc")
		.replace("A native Write or Edit", "A native apply_patch")
		.replace("Bash build or test run", "exec_command build or test run")
		.replace("Bash stays", "Shell execution stays")
		+ "\nCodex hooks bind every git-fs MCP call to this session and workspace. Leave _git_fs to the hook. Stop and SessionEnd preserve the shared checkout; use gitfs_snapshot to build; native shell writes are advisory, not sandboxed."
}

pub fn is_own_tool(payload: &Value) -> bool {
	payload["tool_name"]
		.as_str()
		.and_then(|name| {
			name.strip_prefix("mcp__git-fs__")
				.or_else(|| name.strip_prefix("mcp__git_fs__"))
		})
		.is_some_and(|tool| crate::TOOLS.contains(&tool))
}

pub fn routed_input(payload: &Value, session: &str) -> Result<Value, String> {
	let cwd = payload["cwd"].as_str().ok_or("Codex hook has no cwd")?;
	if !Path::new(cwd).is_absolute() {
		return Err("Codex hook cwd must be absolute".into());
	}
	let mut input = payload["tool_input"]
		.as_object()
		.cloned()
		.unwrap_or_default();
	input.insert(CONTEXT.into(), json!({"session": session, "cwd": cwd}));
	Ok(Value::Object(input))
}

pub fn describe_context(schema: &mut Value) {
	schema["properties"][CONTEXT] = json!({
		"type": "object",
		"description": "Host routing context. Set automatically by the Codex PreToolUse hook; omit from model calls.",
		"properties": {"session": {"type": "string"}, "cwd": {"type": "string"}},
		"required": ["session", "cwd"], "additionalProperties": false
	});
}

pub fn take_context(input: &mut Value, call: u64) -> Result<Context, String> {
	let routing = input.as_object_mut().and_then(|map| map.remove(CONTEXT))
		.ok_or("git-fs Codex routing is missing. Enable and trust this plugin's hooks in /hooks before using its tools.")?;
	if routing.as_object().is_none_or(|fields| fields.len() != 2) {
		return Err("invalid git-fs Codex routing fields".into());
	}
	let session = routing["session"]
		.as_str()
		.filter(|s| crate::provenance::valid_session(s))
		.ok_or("invalid git-fs Codex session")?;
	let cwd = routing["cwd"]
		.as_str()
		.filter(|s| Path::new(s).is_absolute())
		.ok_or("invalid git-fs Codex workspace")?;
	let context = Context {
		session: session.into(),
		cwd: cwd.into(),
		run: format!("mcp-{}", std::process::id()),
		call: call.to_string(),
	};
	context.validate()?;
	Ok(context)
}

fn normalized(path: &Path) -> PathBuf {
	let mut result = PathBuf::new();
	for part in path.components() {
		match part {
			Component::CurDir => {}
			Component::ParentDir => {
				result.pop();
			}
			other => result.push(other.as_os_str()),
		}
	}
	result
}

/// Extract all patch targets, including both sides of a move. Headers alone
/// start with `*** `; hunk contents carry a diff prefix, so cannot add targets.
pub fn patch_paths(payload: &Value, root: &Path, cwd: &Path, store_dir: &str) -> Vec<String> {
	let patch = payload["tool_input"]["command"]
		.as_str()
		.unwrap_or_default();
	let mut paths = Vec::new();
	for line in patch.lines() {
		let Some(path) = [
			"*** Add File: ",
			"*** Update File: ",
			"*** Delete File: ",
			"*** Move to: ",
		]
		.iter()
		.find_map(|prefix| line.strip_prefix(prefix)) else {
			continue;
		};
		let path = Path::new(path.trim());
		let abs = normalized(&if path.is_absolute() {
			path.to_path_buf()
		} else {
			cwd.join(path)
		});
		let Ok(rel) = abs.strip_prefix(root) else {
			continue;
		};
		if rel.as_os_str().is_empty() || rel.starts_with(store_dir) {
			continue;
		}
		if let Some(rel) = rel.to_str() {
			if !paths.iter().any(|p| p == rel) {
				paths.push(rel.to_owned());
			}
		}
	}
	paths
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn host_hook_overwrites_model_supplied_route_before_mcp() {
		let cwd = std::env::current_dir()
			.unwrap()
			.to_string_lossy()
			.into_owned();
		let payload = json!({
			"cwd": cwd,
			"tool_input": {
				"_git_fs": {"session": "spoofed", "cwd": "/wrong"},
				"paths": ["keep.txt"],
			},
		});
		let mut routed = routed_input(&payload, "verified-session").unwrap();
		let context = take_context(&mut routed, 1).unwrap();
		assert_eq!(context.session, "verified-session");
		assert_eq!(context.cwd, cwd);
		assert!(
			routed.get("_git_fs").is_none(),
			"host route is not a model input"
		);
		assert_eq!(routed["paths"], json!(["keep.txt"]));
	}

	#[test]
	fn unknown_or_missing_hook_routing_fields_are_refused() {
		let cwd = std::env::current_dir()
			.unwrap()
			.to_string_lossy()
			.into_owned();
		for mut input in [
			json!({}),
			json!({"_git_fs":{"session":"s","cwd":cwd,"model_override":true}}),
		] {
			assert!(take_context(&mut input, 1).is_err());
		}
	}
}
