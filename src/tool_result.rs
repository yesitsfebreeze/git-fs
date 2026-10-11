//! Existing tool wire contract shared by both producing services.
use crate::Result;
use serde_json::{json, Value};

/// The tool event answers `{content, error}` with the answer itself in `content`
/// — an object for structured answers, a plain string for text and messages,
/// never stringified JSON a consumer must parse twice.
pub(crate) fn success(content: Value) -> Result<Value> {
	Ok(json!({"content":content,"error":false}))
}

pub(crate) fn failure(error: String) -> Value {
	json!({"content":error,"error":true})
}

/// Validate before opening the Git backend or creating its store directory.
/// Existing operation-specific parsing still owns execution and partial results.
pub(crate) fn validate(tool: &str, input: &Value) -> Result<()> {
	let object = input.as_object().ok_or("tool input must be an object")?;
	let op = input["op"].as_str().ok_or("tool op required")?;
	let (required, optional): (&[&str], &[&str]) = match (tool, op) {
		("gitfs", "diff") => (&[], &["path", "offset", "limit", "expected_revision"]),
		("gitfs", "ls") => (&[], &["session"]),
		("gitfs", "rollback") => (&[], &["commit", "path"]),
		("gitfs", "catchup") => (&[], &[]),
		("integrate", "undo") => (&[], &[]),
		("gitfs", "materialize") => (&[], &["paths", "force"]),
		("gitfs", "capture") | ("integrate", "scan") => (&[], &["paths"]),
		("integrate", "push") if input["phase"].as_str() == Some("reconcile") => {
			(&["phase", "operation_id"], &[])
		}
		// a preview derives what it is not given (`push::derive`)
		("integrate", "push") => (
			&[],
			&[
				"remote",
				"ref",
				"commit",
				"expected_remote_head",
				"phase",
				"operation_id",
				"expected_revision",
			],
		),
		("integrate", "integrate") => (
			&[],
			&["message", "force_secrets", "phase", "expected_revision"],
		),
		_ => return Err(format!("unknown {tool} op: {op}")),
	};
	let missing: Vec<&str> = required
		.iter()
		.copied()
		.filter(|key| !object.contains_key(*key))
		.collect();
	if !missing.is_empty() {
		return Err(format!("{op}: {} required", missing.join(", ")));
	}
	if (tool, op) == ("gitfs", "rollback")
		&& object.contains_key("commit") == object.contains_key("path")
	{
		return Err("rollback: give exactly one of `commit` or `path`".into());
	}
	for (key, value) in object {
		if key == "op" {
			continue;
		}
		if !required.contains(&key.as_str()) && !optional.contains(&key.as_str()) {
			let takes = || {
				let fields: Vec<&str> = required.iter().chain(optional).copied().collect();
				match fields.is_empty() {
					true => format!("; {op} takes no fields"),
					false => format!("; {op} takes {}", fields.join(", ")),
				}
			};
			let hint = match (tool, op) {
				("gitfs", "rollback") => ": it takes `commit`, the sha of one of your session's commits (tool.gitfs.ls names them), newest first, or `path` to put one path back to session start".into(),
				("gitfs", "ls") => ": ls lists your session branch (tip, owned paths, commits), not a directory; list files with glob".into(),
				// A tool event reads the session from `context`; callers keep
				// putting it in `input` (FAILURES rows 297, 301).
				_ if key == "session" => format!(": session goes in `context`, not `input`{}", takes()),
				_ => takes(),
			};
			return Err(format!("{key} is not valid for {tool} {op}{hint}"));
		}
		match key.as_str() {
			"force" | "force_secrets" if value.is_boolean() => {}
			"paths" => {
				let paths = value.as_array().ok_or("paths must be an array")?;
				for value in paths {
					valid_path(value.as_str().ok_or("paths must contain strings")?)?;
				}
			}
			"path" => valid_path(value.as_str().ok_or("path must be a string")?)?,
			"message" | "phase" | "expected_revision" | "remote" | "ref" | "commit"
			| "operation_id" | "session"
				if value.is_string() => {}
			"expected_remote_head" if value.is_null() || value.is_string() => {}
			"offset" if value.as_u64().is_some() => {}
			"limit" if value.as_u64().is_some_and(|n| (1..=65536).contains(&n)) => {}
			_ => return Err(format!("invalid type for {key}")),
		}
	}
	Ok(())
}

fn valid_path(path: &str) -> Result<()> {
	if path.is_empty()
		|| path.starts_with('/')
		|| path.contains('\0')
		|| path.split('/').any(|part| part == ".." || part == ".")
	{
		return Err(format!("invalid path: {path}"));
	}
	Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/tool_result.rs"]
mod tests;
