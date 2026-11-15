//! Read-only identity of the running binary, workspace and advertised MCP API.
//! Unavailable revisions are explicit nulls, never inferred from Git HEAD.

use crate::store::{Store, StoreConfig};
use crate::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

fn path_text(path: &Path) -> String {
	path.canonicalize()
		.unwrap_or_else(|_| path.to_path_buf())
		.to_string_lossy()
		.replace('\\', "/")
}

fn canonical_json(value: &Value, output: &mut String) {
	match value {
		Value::Object(fields) => {
			output.push('{');
			let mut entries = fields.iter().collect::<Vec<_>>();
			entries.sort_by_key(|(left, _)| *left);
			for (index, (key, field)) in entries.into_iter().enumerate() {
				if index != 0 {
					output.push(',');
				}
				output.push_str(&serde_json::to_string(key).expect("field name serializes"));
				output.push(':');
				canonical_json(field, output);
			}
			output.push('}');
		}
		Value::Array(items) => {
			output.push('[');
			for (index, item) in items.iter().enumerate() {
				if index != 0 {
					output.push(',');
				}
				canonical_json(item, output);
			}
			output.push(']');
		}
		_ => output.push_str(&serde_json::to_string(value).expect("JSON value serializes")),
	}
}

fn fingerprint_of(declarations: &[Value]) -> String {
	let mut tools = declarations
		.iter()
		.map(|tool| json!({"name":tool["name"], "inputSchema":tool["inputSchema"]}))
		.collect::<Vec<_>>();
	tools.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
	let mut canonical = String::new();
	canonical_json(&Value::Array(tools), &mut canonical);
	format!("sha256:{:x}", Sha256::digest(canonical.as_bytes()))
}

fn fingerprint(codex: bool) -> String {
	fingerprint_of(&crate::mcp::tools(codex))
}

fn operation_mode() -> &'static str {
	match crate::configured_mode().ok() {
		Some(crate::service::Mode::Git) => "git",
		Some(crate::service::Mode::Local) => "local",
		None => "unknown",
	}
}

fn with_common_fields(codex: bool, details: Value) -> Value {
	let mut identity = json!({
		"schema_version": 1,
		"build": {
			"package_version": env!("CARGO_PKG_VERSION"),
			"revision": option_env!("GIT_FS_BUILD_REVISION"),
			"time": option_env!("GIT_FS_BUILD_TIME"),
		},
		"supported_protocol_versions": [crate::mcp::PROTOCOL],
		"tool_schema_fingerprint": fingerprint(codex),
		"tool_schema_profile": if codex { "codex" } else { "standard" },
	});
	identity
		.as_object_mut()
		.expect("identity object")
		.extend(details.as_object().expect("identity fields").clone());
	identity
}

fn unbound() -> Value {
	let mut unknowns = vec![
		"workspace_root",
		"store_root",
		"repository_mode",
		"session.id",
		"revisions.head",
		"revisions.session_overlay",
		"revisions.shared_live",
	];
	if operation_mode() == "unknown" {
		unknowns.push("operation_mode");
	}
	if option_env!("GIT_FS_BUILD_REVISION").is_none() {
		unknowns.push("build.revision");
	}
	if option_env!("GIT_FS_BUILD_TIME").is_none() {
		unknowns.push("build.time");
	}
	with_common_fields(
		true,
		json!({
			"binding_state": "unbound",
			"workspace_root": null,
			"store_root": null,
			"repository_mode": "unknown",
			"operation_mode": operation_mode(),
			"session": {"id": null, "source": "unbound"},
			"revisions": {"head": null, "session_overlay": null, "shared_live": null},
			"unknowns": unknowns,
		}),
	)
}

/// An uninitialized workspace is valid. Other lookup failures are not.
pub(crate) fn read_existing_store(config: &StoreConfig, cwd: &Path) -> Result<Option<Store>> {
	match config.at_readonly(cwd) {
		Ok(store) => Ok(Some(store)),
		Err(error) if error == "no git repository and no git-fs store here yet; the first write creates one" => Ok(None),
		Err(error) => Err(error),
	}
}

fn make(root: &Path, store: Option<&Store>, session: &str, source: &str, codex: bool) -> Value {
	// Direct in-process inspection also works without global `start()`.
	let store_root = crate::store_config().ok().and_then(|config| {
		let dir = root.join(config.directory());
		dir.is_dir().then(|| path_text(&dir))
	});
	let repository_mode = if root.join(".git").exists() {
		"git"
	} else if store.is_some() {
		"standalone"
	} else {
		"uninitialized"
	};
	let operation_mode = operation_mode();
	let head = store.and_then(|store| store.head().ok().flatten());
	let session_tip = store.and_then(|store| store.tip(session).ok().flatten());
	// `live_tip` reconciles refs and can write; runtime inspection must not.
	let live_tip = store.and_then(|store| {
		store
			.git_str(&[
				"rev-parse",
				"--verify",
				"--quiet",
				"refs/gitfs-live/current",
			])
			.ok()
			.map(|revision| revision.trim().to_owned())
			.filter(|revision| !revision.is_empty())
	});
	let build_revision = option_env!("GIT_FS_BUILD_REVISION");
	let build_time = option_env!("GIT_FS_BUILD_TIME");
	let unknowns = [
		("store_root", store_root.is_none()),
		("operation_mode", operation_mode == "unknown"),
		("revisions.head", head.is_none()),
		("revisions.session_overlay", session_tip.is_none()),
		("revisions.shared_live", live_tip.is_none()),
		("build.revision", build_revision.is_none()),
		("build.time", build_time.is_none()),
	]
	.into_iter()
	.filter_map(|(field, unknown)| unknown.then_some(field))
	.collect::<Vec<_>>();
	with_common_fields(
		codex,
		json!({
			"binding_state": "bound",
			"workspace_root": path_text(root),
			"store_root": store_root,
			"repository_mode": repository_mode,
			"operation_mode": operation_mode,
			"session": {"id": session, "source": source},
			"revisions": {"head": head, "session_overlay": session_tip, "shared_live": live_tip},
			"unknowns": unknowns,
		}),
	)
}

pub(crate) fn inspect(codex: bool) -> Result<Value> {
	// Codex is routed per call; before the first trusted hook context the
	// process directory and environment do not identify its target.
	if codex {
		return Ok(unbound());
	}
	let workspace = crate::mcp::workspace()?
		.canonicalize()
		.map_err(|error| format!("workspace unavailable: {error}"))?;
	let (session, source) = crate::mcp::resolved_session()?;
	let config = crate::store_config()?;
	let store = read_existing_store(&config, &workspace)?;
	let root = store
		.as_ref()
		.map(|store| store.root())
		.unwrap_or(&workspace);
	Ok(make(root, store.as_ref(), &session, source, codex))
}

pub(crate) fn for_store(store: &Store, session: &str, source: &str) -> Value {
	make(store.root(), Some(store), session, source, false)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fingerprint_ignores_tool_order_but_tracks_schema_changes() {
		let mut tools = crate::mcp::tools(false);
		let before = fingerprint_of(&tools);
		tools.reverse();
		assert_eq!(fingerprint_of(&tools), before);
		tools[0]["inputSchema"]["x-test-extra-field"] = json!(true);
		assert_ne!(fingerprint_of(&tools), before);
	}
}
