//! The Git operations: each edit has a private session commit and a durable
//! shared live commit. This tool inspects
//! that branch (`ls`, `diff`), undoes a commit (`rollback`), copies owned
//! files into the repository worktree, the one place builds and tests run
//! (`materialize`), and pulls disk edits made outside the tools back in
//! (`capture`). Materialize refuses to clobber on-disk files newer than the
//! copy.

#[path = "inspection.rs"]
mod inspection;

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
		"description":"The session's Git branch: writes and edits commit to refs/gitfs/<session>, and the worktree stays untouched until materialize copies files in to run them; rollback undoes one commit, or puts one path back to session start; catchup moves the session onto trunk after trunk moved (replays paths trunk did not touch, drops ones trunk already holds, merges non-overlapping edits from two sessions, refuses only overlapping edits by path, empties the session when nothing is left). Ops: ls, diff, rollback, catchup, materialize, capture, snapshot. Full guidance: `kern help fs`.",
		"input_schema":{
			"type":"object",
			"required":["op"],
			"properties":{
				"op":{"type":"string","enum":["ls","diff","rollback","catchup","materialize","capture","snapshot"]},
				"path":{"type":"string","minLength":1,"description":"Repo-relative path, no leading / or ..; absent answers the session-wide summary: every owned path with its status"},
				"session":{"type":"string","minLength":1,"description":"Whose branch to list (ls); your own by default."},
				"commit":{"type":"string","minLength":1,"description":"A commit of your session to undo (rollback; or give `path` instead to put that path back to its content at session start)."},
				"paths":{"type":"array","items":{"type":"string"},"description":"Restrict materialization to these owned paths; explicit capture paths adopt selected disk files and tracked deletions."},
				"force":{"type":"boolean","description":"Overwrite newer on-disk files (materialize)."},
				"offset":{"type":"integer","minimum":0},
				"limit":{"type":"integer","minimum":1,"maximum":65536,"description":"Patch bytes per page (diff); below 128 a page still answers 128, so a page always moves past a multibyte character."},
				"expected_revision":{"type":"string","description":"Required for diff continuation."},
				"id":{"type":"string","description":"Release this session's previously generated snapshot; omit to create one."},
				"kind":{"type":"string","enum":["live","candidate"],"description":"Snapshot kind; defaults to live for backward compatibility."},
				"expected_tree":{"type":"string","minLength":40,"description":"Require an exact integration-candidate tree ID."},
				"expected_review":{"type":"object","description":"Require the exact ReviewedTree returned by an integration preview."},
				"capability":{"type":"string","minLength":64,"description":"Unforgeable per-run capability returned on candidate creation; required for its release."}
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
				Some("rollback" | "catchup" | "capture" | "materialize" | "snapshot")
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
/// Mirror a rollback as another durable shared commit when that session's
/// previous version is still current. Never undo a newer peer's change.
fn publish_rollback(
	store: &crate::store::Store,
	session: &str,
	rel: &str,
	before: Option<&[u8]>,
) -> Result<()> {
	let path = Path::new(rel);
	let after = store.read(session, path)?;
	let live = store.live_content(path)?;
	if live.as_deref() == before {
		let mode = store.mode_for(session, path)?;
		store.publish_live_rollback(session, path, before, after.as_deref(), &mode)?;
	} else if live != after {
		return Err(format!("rollback of {rel} conflicts with a newer live edit; reread before changing shared state"));
	}
	Ok(())
}

pub(crate) fn dispatch_captured(
	store: &crate::store::Store,
	session: &str,
	input: &Value,
	capture: &mut Capture,
) -> Result<Value> {
	match input["op"].as_str() {
		Some("ls") => {
			let inspected_session = match input["session"].as_str() {
				Some(other) if !crate::provenance::valid_session(other) => {
					return err(format!("ls: invalid session {other}"))
				}
				Some(other) => other,
				None => session,
			};
			let tip = store.tip(inspected_session)?;
			let owned = store.owned_paths(inspected_session)?;
			let runtime = crate::runtime_identity::for_store(store, session, "invocation_context");
			ok(json!({
				"session":inspected_session,
				"runtime":runtime,
				"branch":store.branch(inspected_session),
				"tip":tip,
				"owned":owned.iter().map(|(s,p)| json!({"status":s.to_string(),"path":p})).collect::<Vec<_>>(),
				"commits":store.session_commits(inspected_session, 50)?,
			}))
		}
		Some("rollback") => {
			if let Some(path) = input["path"].as_str() {
				let before = store.read(session, Path::new(path))?;
				store.rollback_path_captured(session, path, capture)?;
				publish_rollback(store, session, path, before.as_deref())?;
				return ok(json!({"rolled_back":path,"restored":[path],"tip":store.tip(session)?}));
			}
			let commit = input["commit"]
				.as_str()
				.ok_or("rollback: commit required")?;
			let mut before = std::collections::HashMap::new();
			for (_, path) in store.session_paths(session)? {
				before.insert(path.clone(), store.read(session, Path::new(&path))?);
			}
			let restored = store.rollback_captured(session, commit, capture)?;
			for path in &restored {
				publish_rollback(
					store,
					session,
					path,
					before.get(path).and_then(|bytes| bytes.as_deref()),
				)?;
			}
			ok(json!({"rolled_back":commit,"restored":restored,"tip":store.tip(session)?}))
		}
		Some("catchup") => ok(store.catchup_captured(session, capture)?),
		Some("snapshot") => match (
			input["kind"].as_str().unwrap_or("live"),
			input["id"].as_str(),
		) {
			("live", Some(id)) => ok(store.release_snapshot(session, id)?),
			("live", None) => ok(store.create_snapshot(session)?),
			("candidate", Some(id)) => {
				let capability = input["capability"]
					.as_str()
					.ok_or("candidate release capability required")?;
				ok(store.release_candidate_snapshot(session, id, capability)?)
			}
			("candidate", None) => ok(store.create_candidate_snapshot_checked(
				session,
				input["expected_tree"].as_str(),
				input.get("expected_review"),
			)?),
			(other, _) => err(format!("unsupported snapshot kind: {other}")),
		},
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
	let mut seen = std::collections::HashSet::new();
	let mut selected = Vec::new();
	for path in requested {
		let rel = if explicit.is_some() {
			capture_path(store.root(), &path)?
		} else {
			path
		};
		if seen.insert(rel.clone()) {
			selected.push(rel);
		}
	}
	if explicit.is_some() {
		let missing: Vec<String> = selected
			.iter()
			.filter(|rel| {
				!store.root().join(*rel).exists() && !owned.iter().any(|(_, path)| path == *rel)
			})
			.cloned()
			.collect();
		if !missing.is_empty() {
			for (rel, tracked) in missing.iter().zip(store.capture_tracks(session, &missing)?) {
				if !tracked {
					return Err(format!(
						"capture: missing path is not tracked by this session or trunk: {rel}"
					));
				}
			}
		}
	}
	let mut readable = Vec::with_capacity(selected.len());
	let mut errors = Vec::new();
	for rel in selected {
		let read = (|| -> Result<Option<Vec<u8>>> {
			capture_path(store.root(), &rel)?;
			match std::fs::read(store.root().join(&rel)) {
				Ok(bytes) => Ok(Some(bytes)),
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
				Err(e) => Err(format!("read failed: {e}")),
			}
		})();
		match read {
			Ok(content) => readable.push((rel, content)),
			Err(error) => errors.push(format!("{rel}: {error}")),
		}
	}
	// One session commit and one live commit for the whole batch.
	let (saved, batch_errors) = store.capture_batch(session, &readable, capture)?;
	errors.extend(batch_errors);
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
