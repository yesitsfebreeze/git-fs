//! The Git session operations: the session branch behind `tool.write` and `tool.edit`. Every
//! change there is one commit on `refs/gitfs/<session>`; this tool inspects
//! that branch (`ls`, `diff`), undoes a commit (`rollback`), copies owned
//! files into the repository worktree, the one place builds and tests run
//! (`materialize`), and pulls disk edits made outside the tools back in
//! (`capture`). Materialize refuses to clobber on-disk files newer than the
//! copy.

#[path = "inspection.rs"]
mod inspection;

use crate::change_record::Operation;
use crate::provenance::{Capture, Recorder};
use crate::store::StoreConfig;
use crate::tool_result::success as ok;
use crate::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

pub(crate) fn describe() -> Value {
	json!({
		"name":"gitfs",
		"description":"The session's Git branch: writes and edits commit to refs/gitfs/<session>, and the worktree stays untouched until materialize copies files in to run them; rollback undoes one commit, or puts one path back to session start; catchup moves the session onto trunk after trunk moved (replays paths trunk did not touch, drops ones trunk already holds, refuses real conflicts by path, empties the session when nothing is left). Ops: ls, diff, rollback, catchup, materialize, capture. Full guidance: `kern help fs`.",
		"input_schema":{
			"type":"object",
			"required":["op"],
			"properties":{
				"op":{"type":"string","enum":["ls","diff","rollback","catchup","materialize","capture"]},
				"path":{"type":"string","minLength":1,"description":"Repo-relative path, no leading / or ..; absent answers the session-wide summary: every owned path with its status"},
				"session":{"type":"string","minLength":1,"description":"Whose branch to list (ls); your own by default."},
				"commit":{"type":"string","minLength":1,"description":"A commit of your session to undo (rollback; or give `path` instead to put that path back to its content at session start)."},
				"paths":{"type":"array","items":{"type":"string"},"description":"Restrict to these owned paths (materialize, capture)."},
				"force":{"type":"boolean","description":"Overwrite newer on-disk files (materialize)."},
				"offset":{"type":"integer","minimum":0},
				"limit":{"type":"integer","minimum":1,"maximum":65536,"description":"Patch bytes per page (diff); below 128 a page still answers 128, so a page always moves past a multibyte character."},
				"expected_revision":{"type":"string","description":"Required for diff continuation."}
			},
			"additionalProperties":false
		}
	})
}

/// Validate a repo-relative path: no escape, no NUL. Returns the normalized
/// path and its absolute location under the repo root.
fn resolve_rel(root: &std::path::Path, path: &str) -> Result<(String, std::path::PathBuf)> {
	if path.is_empty()
		|| path.starts_with('/')
		|| path.contains('\0')
		|| path.split('/').any(|c| c == ".." || c == ".")
	{
		return Err(format!("invalid path: {path}"));
	}
	let abs = root.join(path);
	if !abs.starts_with(root) {
		return Err(format!("path escapes the repository: {path}"));
	}
	Ok((path.to_owned(), abs))
}

fn session_of(args: &Value) -> Result<String> {
	args["context"]["session"]
		.as_str()
		.filter(|s| !s.is_empty())
		.map(str::to_owned)
		.ok_or("session required".into())
}

fn err(content: impl Into<String>) -> Result<Value> {
	Ok(json!({"content": content.into(), "error": true}))
}

#[cfg(test)]
pub async fn handle(cfg: Arc<StoreConfig>, args: Value) -> Result<Value> {
	handle_with_recorder(cfg, args, Recorder::Absent).await
}
pub(crate) async fn handle_with_recorder(
	cfg: Arc<StoreConfig>,
	args: Value,
	recorder: Recorder,
) -> Result<Value> {
	match args["op"].as_str() {
		Some("describe") => Ok(describe()),
		Some("cancel") => Ok(
			json!({"content":"invocation cancelled; completed changes are not rolled back","error":false}),
		),
		Some("call") => {
			crate::tool_result::validate("gitfs", &args["input"])?;
			let session = session_of(&args)?;
			let cwd = args["context"]["cwd"]
				.as_str()
				.map(std::path::PathBuf::from)
				.ok_or_else(|| "cwd required".to_string())?;
			if args["context"]["cwd"]
				.as_str()
				.is_some_and(|cwd| cwd.is_empty() || cwd.len() > 4096 || cwd.contains('\0'))
			{
				return Err("invalid attribution cwd".into());
			}
			let mutation = matches!(
				args["input"]["op"].as_str(),
				Some("rollback" | "catchup" | "capture" | "materialize")
			);
			let mut capture = if mutation {
				Capture::from_context(&args["context"])?
			} else {
				Capture::default()
			};
			let store = if matches!(args["input"]["op"].as_str(), Some("ls" | "diff")) {
				cfg.at_readonly(&cwd)?
			} else {
				cfg.at(&cwd)?
			};
			let input = args["input"].clone();
			let (outcome, capture) = tokio::task::spawn_blocking(move || {
				let result = dispatch_captured(&store, &session, &input, &mut capture);
				(result, capture)
			})
			.await
			.map_err(|e| format!("mutation worker outcome unknown: {e}"))?;
			if mutation {
				recorder.finish(outcome, capture).await
			} else {
				outcome
			}
		}
		Some(other) => err(format!("unknown gitfs op: {other}")),
		None => err("gitfs op required"),
	}
}

#[cfg(test)]
fn dispatch(store: &crate::store::Store, session: &str, input: &Value) -> Result<Value> {
	dispatch_captured(store, session, input, &mut Capture::default())
}
pub(crate) fn dispatch_captured(
	store: &crate::store::Store,
	session: &str,
	input: &Value,
	capture: &mut Capture,
) -> Result<Value> {
	match input["op"].as_str() {
		Some("ls") => {
			let session = match input["session"].as_str() {
				Some(other) if !crate::provenance::valid_session(other) => {
					return err(format!("ls: invalid session {other}"))
				}
				Some(other) => other,
				None => session,
			};
			let tip = store.tip(session)?;
			let owned = store.owned_paths(session)?;
			ok(json!({
				"session":session,
				"branch":store.branch(session),
				"tip":tip,
				"owned":owned.iter().map(|(s,p)| json!({"status":s.to_string(),"path":p})).collect::<Vec<_>>(),
				"commits":store.session_commits(session, 50)?,
			}))
		}
		Some("rollback") => {
			if let Some(path) = input["path"].as_str() {
				store.rollback_path_captured(session, path, capture)?;
				return ok(json!({"rolled_back":path,"restored":[path],"tip":store.tip(session)?}));
			}
			let commit = input["commit"]
				.as_str()
				.ok_or("rollback: commit required")?;
			let restored = store.rollback_captured(session, commit, capture)?;
			ok(json!({"rolled_back":commit,"restored":restored,"tip":store.tip(session)?}))
		}
		Some("catchup") => ok(store.catchup_captured(session, capture)?),
		Some("diff") => inspection::diff(store, session, input),
		Some("materialize") => {
			let paths = input["paths"].as_array().map(|a| {
				a.iter()
					.filter_map(Value::as_str)
					.map(str::to_owned)
					.collect::<Vec<_>>()
			});
			let force = input["force"].as_bool().unwrap_or(false);
			let out = store.materialize_captured(session, paths.as_deref(), force, capture)?;
			let mut value = ok(out.to_value())?;
			value["error"] = json!(!out.errors.is_empty());
			Ok(value)
		}
		Some("capture") => capture_paths(store, session, input, capture),
		Some(other) => err(format!("unknown gitfs op: {other}")),
		None => err("gitfs op required"),
	}
}

fn capture_paths(
	store: &crate::store::Store,
	session: &str,
	input: &Value,
	capture: &mut Capture,
) -> Result<Value> {
	let owned = store.owned_paths(session)?;
	let selected = input["paths"].as_array();
	if let Some(paths) = selected {
		for path in paths {
			let path = path.as_str().ok_or("capture paths must contain strings")?;
			resolve_rel(store.root(), path)?;
			if !owned.iter().any(|(_, rel)| rel == path) {
				return Err(format!(
					"capture: path is not owned by this session: {path}"
				));
			}
		}
	}
	let source = store.root();
	let mut saved = Vec::new();
	let mut errors = Vec::new();
	for (status, rel) in owned {
		if selected.is_some_and(|paths| !paths.iter().any(|p| p.as_str() == Some(&rel))) {
			continue;
		}
		if status == 'D' {
			continue;
		}
		let capture = (|| -> Result<bool> {
			let path = Path::new(&rel);
			let bytes =
				std::fs::read(source.join(&rel)).map_err(|e| format!("read failed: {e}"))?;
			let mode = store.mode_for(session, path)?;
			Ok(store
				.write_blob_captured(session, path, &bytes, &mode, capture, Operation::Capture)?
				.is_some())
		})();
		match capture {
			Ok(true) => saved.push(rel),
			Ok(false) => {}
			Err(error) => errors.push(format!("{rel}: {error}")),
		}
	}
	let mut result = ok(json!({"captured":saved,"errors":errors}))?;
	result["error"] = json!(!errors.is_empty());
	Ok(result)
}

#[cfg(test)]
#[path = "../tests/unit/capture.rs"]
mod capture_tests;
