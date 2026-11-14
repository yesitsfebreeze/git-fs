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
		"description":"The session's Git branch: writes and edits commit to refs/gitfs/<session>, and the worktree stays untouched until materialize copies files in to run them; rollback undoes one commit, or puts one path back to session start; catchup moves the session onto trunk after trunk moved (replays paths trunk did not touch, drops ones trunk already holds, merges non-overlapping edits from two sessions, refuses only overlapping edits by path, empties the session when nothing is left). Ops: ls, diff, rollback, catchup, materialize, capture. Full guidance: `kern help fs`.",
		"input_schema":{
			"type":"object",
			"required":["op"],
			"properties":{
				"op":{"type":"string","enum":["ls","diff","rollback","catchup","materialize","capture"]},
				"path":{"type":"string","minLength":1,"description":"Repo-relative path, no leading / or ..; absent answers the session-wide summary: every owned path with its status"},
				"session":{"type":"string","minLength":1,"description":"Whose branch to list (ls); your own by default."},
				"commit":{"type":"string","minLength":1,"description":"A commit of your session to undo (rollback; or give `path` instead to put that path back to its content at session start)."},
				"paths":{"type":"array","items":{"type":"string"},"description":"Restrict materialization to these owned paths; explicit capture paths adopt selected disk files and tracked deletions."},
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
	let owned = store.session_paths(session)?;
	let explicit = input["paths"].as_array();
	let requested: Vec<String> = match explicit {
		Some(paths) => paths
			.iter()
			.map(|p| {
				p.as_str()
					.map(str::to_owned)
					.ok_or("capture paths must contain strings".to_owned())
			})
			.collect::<Result<_>>()?,
		None => owned.iter().map(|(_, path)| path.clone()).collect(),
	};
	let mut selected = Vec::new();
	for path in requested {
		let rel = if explicit.is_some() {
			capture_path(store.root(), &path)?
		} else {
			path
		};
		if selected.contains(&rel) {
			continue;
		}
		if explicit.is_some()
			&& !store.root().join(&rel).exists()
			&& !owned.iter().any(|(_, p)| p == &rel)
			&& store.selected_blob(session, Path::new(&rel))?.is_none()
		{
			return Err(format!(
				"capture: missing path is not tracked by this session or trunk: {rel}"
			));
		}
		selected.push(rel);
	}
	let mut saved = Vec::new();
	let mut errors = Vec::new();
	for rel in selected {
		let result = (|| -> Result<bool> {
			capture_path(store.root(), &rel)?;
			let path = Path::new(&rel);
			let changed = match std::fs::read(store.root().join(&rel)) {
				Ok(bytes) => {
					let mode = store.mode_for(session, path)?;
					store.write_blob_captured(
						session,
						path,
						&bytes,
						&mode,
						capture,
						Operation::Capture,
					)?
				}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
					store.delete_blob_captured(session, path, capture, Operation::Capture)?
				}
				Err(e) => return Err(format!("read failed: {e}")),
			};
			store.note_materialized(session, &rel)?;
			Ok(changed.is_some())
		})();
		match result {
			Ok(true) => saved.push(rel),
			Ok(false) => {}
			Err(error) => errors.push(format!("{rel}: {error}")),
		}
	}
	let mut result = ok(json!({"captured":saved,"errors":errors}))?;
	result["error"] = json!(!errors.is_empty());
	Ok(result)
}

/// Explicit capture is an opt-in adoption of disk content, never an escape
/// through a symlink or a recursive directory operation.
fn capture_path(root: &Path, path: &str) -> Result<String> {
	use std::path::Component;
	let path = Path::new(path);
	if path.as_os_str().is_empty()
		|| path
			.components()
			.any(|c| !matches!(c, Component::Normal(_)))
	{
		return Err("capture requires a repository-relative file path without . or ..".into());
	}
	let root = root.canonicalize().map_err(|e| e.to_string())?;
	let rel = path
		.components()
		.map(|c| c.as_os_str().to_str().ok_or("capture path is not UTF-8"))
		.collect::<Result<Vec<_>, _>>()?
		.join("/");
	let mut part = root.clone();
	for component in path.components() {
		part.push(component);
		match std::fs::symlink_metadata(&part) {
			Ok(metadata) if metadata.file_type().is_symlink() => {
				return Err(format!("capture refuses symbolic links: {rel}"))
			}
			Ok(metadata) if part == root.join(path) && !metadata.is_file() => {
				return Err(format!("capture requires a regular file: {rel}"))
			}
			Ok(_) => (),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
			Err(e) => return Err(format!("capture path unavailable: {e}")),
		}
	}
	if crate::files::resolve(&root, path)? != root.join(path) {
		return Err(format!(
			"capture path resolves outside its selected location: {rel}"
		));
	}
	Ok(rel)
}

#[cfg(test)]
#[path = "../tests/unit/capture.rs"]
mod capture_tests;
