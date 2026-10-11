//! Claude Code lifecycle hooks. `session-start` pins the session the MCP
//! server serves; `post-edit` mirrors a native Write or Edit onto the session
//! branch; `read` puts a path the session changed onto disk before a native
//! Read sees it stale; `stop` materializes every owned path. A hook never
//! fails the session: errors go to stderr and the exit code stays 0.

use crate::provenance::Capture;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

fn payload() -> Value {
	let mut text = String::new();
	let _ = std::io::stdin().read_to_string(&mut text);
	serde_json::from_str(&text).unwrap_or(Value::Null)
}

fn note(text: &str) {
	eprintln!("[git-fs] {text}");
}

fn file_path(payload: &Value) -> Option<PathBuf> {
	let input = &payload["tool_input"];
	input["file_path"]
		.as_str()
		.or(input["path"].as_str())
		.map(PathBuf::from)
}

fn relative(root: &Path, path: &Path) -> Option<String> {
	let abs = if path.is_absolute() {
		path.to_path_buf()
	} else {
		root.join(path)
	};
	let rel = abs.strip_prefix(root).ok()?;
	let rel = rel.to_str()?;
	(!rel.is_empty() && !rel.split('/').any(|c| c == "..")).then(|| rel.to_owned())
}

fn run_inner(action: &str) -> Result<(), String> {
	let payload = payload();
	let session = payload["session_id"]
		.as_str()
		.filter(|s| crate::provenance::valid_session(s))
		.ok_or("no session_id in the hook payload")?
		.to_owned();
	crate::start(crate::env_config()?)?;
	let config = crate::store_config()?;
	let cwd = payload["cwd"]
		.as_str()
		.map(PathBuf::from)
		.map(Ok)
		.unwrap_or_else(crate::mcp::workspace)?;
	let store = match config.at(&cwd) {
		Ok(store) => store,
		Err(_) => return Ok(()), // outside a repository git-fs has nothing to do
	};
	let root = store.root().to_path_buf();
	match action {
		"session-start" => {
			std::fs::write(
				root.join(config.directory()).join("CURRENT"),
				format!("{session}\n"),
			)
			.map_err(|e| format!("cannot pin the session: {e}"))?;
			let owned = store.owned_paths(&session)?.len();
			note(&format!(
				"session {session} on refs/gitfs/{session}: {owned} owned path(s); writes through the git-fs tools stay on the branch until gitfs_materialize"
			));
		}
		"post-edit" => {
			let Some(path) = file_path(&payload) else {
				return Ok(());
			};
			let Some(rel) = relative(&root, &path) else {
				return Ok(());
			};
			if rel.starts_with(config.directory()) {
				return Ok(());
			}
			let bytes = match std::fs::read(root.join(&rel)) {
				Ok(bytes) => bytes,
				Err(_) => return Ok(()), // gone again: nothing to mirror
			};
			let mode = store.mode_for(&session, Path::new(&rel))?;
			if store.read(&session, Path::new(&rel))?.as_deref() == Some(bytes.as_slice()) {
				return Ok(());
			}
			store.write_blob(&session, Path::new(&rel), &bytes, &mode)?;
			store.note_materialized(&session, &rel)?;
		}
		"read" => {
			let Some(path) = file_path(&payload) else {
				return Ok(());
			};
			let Some(rel) = relative(&root, &path) else {
				return Ok(());
			};
			if !store
				.owned_paths(&session)?
				.iter()
				.any(|(_, owned)| owned == &rel)
			{
				return Ok(());
			}
			let branch = store.read(&session, Path::new(&rel))?;
			let disk = std::fs::read(root.join(&rel)).ok();
			if branch == disk {
				return Ok(());
			}
			let result = store.materialize_captured(
				&session,
				Some(std::slice::from_ref(&rel)),
				false,
				&mut Capture::default(),
			)?;
			for (path, why) in &result.conflicts {
				note(&format!("{path} left on disk as it is: {why}; the branch holds your version, gitfs_materialize with force to take it"));
			}
		}
		"stop" => {
			if store.tip(&session)?.is_none() {
				return Ok(());
			}
			let result =
				store.materialize_captured(&session, None, false, &mut Capture::default())?;
			if !result.applied.is_empty() {
				note(&format!(
					"materialized {} path(s) from refs/gitfs/{session}",
					result.applied.len()
				));
			}
			for (path, why) in &result.conflicts {
				note(&format!("{path} left on disk as it is: {why}"));
			}
			for error in &result.errors {
				note(error);
			}
		}
		other => return Err(format!("unknown hook action: {other}")),
	}
	Ok(())
}

pub fn run(action: &str) -> i32 {
	if let Err(e) = run_inner(action) {
		note(&format!("hook {action}: {e}"));
	}
	0
}
