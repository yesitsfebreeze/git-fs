//! Claude Code and Codex lifecycle hooks. `session-start` pins the session the MCP
//! server serves and hands the model a short guide to the git-fs tools;
//! `pre-tool` steers a native file tool to its git-fs counterpart (a `Write`
//! or `Edit` inside the repository is refused with the reason, a `Read`,
//! `Grep`, `Glob` or file-reading `Bash` call gets a note next to its result)
//! and puts a path the session changed onto disk before a native `Read` sees
//! it stale; `post-edit` mirrors a native Write or Edit that did run onto the
//! session branch; `stop` materializes every owned path. A hook never fails
//! the session: errors go to stderr and the exit code stays 0.

use crate::provenance::Capture;
use crate::service::Mode;
use serde_json::{json, Value};
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

/// One JSON object on stdout: Claude Code reads `hookSpecificOutput` from it.
fn emit(event: &str, fields: Value) {
	let mut output = json!({"hookEventName": event});
	crate::config::merge(&mut output, fields);
	println!("{}", json!({"hookSpecificOutput": output}));
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

/// What the model gets at session start, as `additionalContext`.
pub fn guide(session: &str, owned: usize, mode: Mode) -> String {
	let mut text = format!(
		"git-fs is on: session {session}, branch refs/gitfs/{session}, {owned} owned path(s). "
	);
	if mode == Mode::Local {
		text.push_str("Mode local: writes land in the worktree directly.\n");
	} else {
		text.push_str(
			"Writes through the git-fs tools stay on the branch until gitfs_materialize.\n",
		);
	}
	text.push_str(
		"Use the git-fs tools (mcp__plugin_git-fs_git-fs__*) for files in this repository, not the native ones:\n\
		 - read, not Read, cat or sed -n: answers from the branch before the worktree and records what you read, which is what makes your next edit fresh.\n\
		 - edit and write, not Edit, Write or a heredoc: one commit each on the branch; refused when the bytes you last read are no longer current. A native Write or Edit inside the repository is blocked by the pre-tool hook with this reminder.\n\
		 - grep, glob and search, not Grep, Glob, rg or find: see the branch, then trunk, then untracked disk files. The native ones see the disk only.\n\
		 - gitfs_materialize before a Bash build or test run: copies owned paths to the worktree (Stop does it too). gitfs_capture pulls disk edits of owned paths back in.\n\
		 - gitfs_ls and gitfs_diff to review, gitfs_catchup after trunk moved, integrate to commit the session onto trunk, push to publish.\n\
		 Bash stays for builds, tests, git and everything that is not reading or writing project files.",
	);
	text
}

/// How the pre-tool hook answers one native tool call.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
	Allow,
	/// Refuse the call; the text is the reason the model reads.
	Deny(String),
	/// Let it run and put the text next to the result.
	Note(String),
}

/// Native `Read` handles these; git-fs `read` has nothing to add.
const BINARY_READS: &[&str] = &[
	"png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "pdf", "ipynb",
];

/// The command words a Bash call uses to read files, and the git-fs tool
/// that answers from the branch instead.
const BASH_READERS: &[(&str, &str)] = &[
	("cat", "read"),
	("head", "read"),
	("tail", "read"),
	("less", "read"),
	("more", "read"),
	("sed", "read"),
	("awk", "read"),
	("bat", "read"),
	("rg", "grep"),
	("grep", "grep"),
	("egrep", "grep"),
	("ag", "grep"),
	("find", "glob"),
	("fd", "glob"),
];

/// The first command of a Bash line after any leading `cd <dir> &&` or `;`,
/// with its arguments.
fn first_command(command: &str) -> Vec<&str> {
	let mut rest = command.trim_start();
	loop {
		let words: Vec<&str> = rest.split_whitespace().collect();
		if words.first() != Some(&"cd") {
			return words;
		}
		let Some(cut) = rest.find("&&").or_else(|| rest.find(';')) else {
			return Vec::new();
		};
		rest = rest[cut..].trim_start_matches(['&', ';']).trim_start();
	}
}

/// The git-fs tool a Bash command should have been, when it only reads or
/// writes project files.
pub fn bash_redirect(command: &str) -> Option<(&'static str, &'static str)> {
	let words = first_command(command);
	let word = *words.first()?;
	let heredoc = command.contains("<<");
	if (word == "cat" || word == "tee") && (heredoc || words.iter().any(|w| w.starts_with('>'))) {
		return Some((if word == "cat" { "cat" } else { "tee" }, "write"));
	}
	BASH_READERS
		.iter()
		.find(|(reader, _)| *reader == word)
		.copied()
}

/// `rel` is the repo-relative path of the call's `file_path`/`path`, `None`
/// when the call names no path inside the repository (for `Grep` and `Glob`
/// the caller passes the repository itself when no path is given).
pub fn verdict(tool: &str, input: &Value, rel: Option<&str>, session: &str) -> Verdict {
	match tool {
		"Write" | "Edit" | "MultiEdit" | "apply_patch" => match rel {
			Some(rel) => Verdict::Deny(format!(
				"git-fs session {session} is on. Use the git-fs edit or write tool for {rel}, after a git-fs read of it: \
				 that commits to refs/gitfs/{session} and refuses a stale edit; native {tool} skips the guard. \
				 Native writes only: GIT_FS_CONFIG='{{\"mode\":\"local\"}}'."
			)),
			None => Verdict::Allow,
		},
		"Read" => match rel {
			Some(rel)
				if !input["pages"].is_string()
					&& !Path::new(rel)
						.extension()
						.and_then(|e| e.to_str())
						.is_some_and(|e| BINARY_READS.contains(&e.to_ascii_lowercase().as_str())) =>
			{
				Verdict::Note(format!(
					"git-fs read {rel} answers from the session branch and records the read, which is what makes a following git-fs edit fresh; native Read saw the disk only."
				))
			}
			_ => Verdict::Allow,
		},
		"Grep" | "Glob" => match rel {
			Some(_) => Verdict::Note(format!(
				"git-fs {} searches the session branch, then trunk, then untracked disk files; native {tool} sees the disk only and misses unmaterialized changes.",
				tool.to_ascii_lowercase()
			)),
			None => Verdict::Allow,
		},
		"Bash" => match input["command"].as_str().and_then(bash_redirect) {
			Some((word, "write")) => Verdict::Note(format!(
				"`{word}` wrote to disk behind the session branch; the git-fs write tool commits to refs/gitfs/{session} and keeps the freshness guard."
			)),
			Some((word, replacement)) => Verdict::Note(format!(
				"`{word}` reads the disk only; git-fs {replacement} sees the session branch too. Bash stays for builds, tests and git."
			)),
			None => Verdict::Allow,
		},
		_ => Verdict::Allow,
	}
}

fn run_inner(action: &str, codex: bool) -> Result<(), String> {
	let payload = payload();
	let session = payload["session_id"]
		.as_str()
		.filter(|s| crate::provenance::valid_session(s))
		.ok_or("no session_id in the hook payload")?
		.to_owned();
	// Codex's tool hook supplies the context for this exact MCP call. Never
	// route it through the checkout-global CURRENT file: another thread can
	// change that file while this one is still running.
	if codex && action == "pre-tool" && crate::codex::is_own_tool(&payload) {
		emit(
			"PreToolUse",
			json!({
				"permissionDecision": "allow",
				"updatedInput": crate::codex::routed_input(&payload, &session)?
			}),
		);
		return Ok(());
	}
	let env = crate::env_config()?;
	let mode = crate::service::Config::parse(env.clone())?.mode;
	let tool = payload["tool_name"].as_str().unwrap_or("").to_owned();
	// A Bash call that is not a file read or write costs nothing: no store.
	if action == "pre-tool"
		&& tool == "Bash"
		&& verdict(&tool, &payload["tool_input"], None, &session) == Verdict::Allow
	{
		return Ok(());
	}
	crate::start(env)?;
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
	let rel_of = |payload: &Value| -> Option<String> {
		let path = file_path(payload)?;
		let abs = if path.is_absolute() {
			path
		} else {
			cwd.join(path)
		};
		let rel = relative(&root, &abs)?;
		(!Path::new(&rel).starts_with(config.directory())).then_some(rel)
	};
	match action {
		"session-start" => {
			if !codex {
				std::fs::write(
					root.join(config.directory()).join("CURRENT"),
					format!("{session}\n"),
				)
				.map_err(|e| format!("cannot pin the session: {e}"))?;
			}
			let owned = store.owned_paths(&session)?.len();
			emit(
				"SessionStart",
				json!({"additionalContext": if codex { crate::codex::guide(&session, owned, mode) } else { guide(&session, owned, mode) }}),
			);
		}
		"post-edit" if codex && tool == "apply_patch" => {
			for rel in crate::codex::patch_paths(&payload, &root, &cwd, config.directory()) {
				mirror(&store, &session, &root, &rel)?;
			}
		}
		"post-edit" => {
			let Some(rel) = rel_of(&payload) else {
				return Ok(());
			};
			mirror(&store, &session, &root, &rel)?;
		}
		"pre-tool" | "read" => {
			let rel = match tool.as_str() {
				"apply_patch" if codex => {
					crate::codex::patch_paths(&payload, &root, &cwd, config.directory())
						.into_iter()
						.next()
				}
				"Grep" | "Glob" if file_path(&payload).is_none() => Some(".".to_owned()),
				"Bash" => None,
				_ => rel_of(&payload),
			};
			if tool == "Read" {
				if let Some(rel) = rel.as_deref() {
					materialize_stale(&store, &session, &root, rel)?;
				}
			}
			if mode == Mode::Local {
				return Ok(()); // the worktree is the truth: nothing to redirect
			}
			match verdict(&tool, &payload["tool_input"], rel.as_deref(), &session) {
				Verdict::Allow => {}
				Verdict::Deny(reason) => emit(
					"PreToolUse",
					json!({"permissionDecision": "deny", "permissionDecisionReason": reason}),
				),
				Verdict::Note(text) => emit("PreToolUse", json!({"additionalContext": text})),
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

fn mirror(
	store: &crate::store::Store,
	session: &str,
	root: &Path,
	rel: &str,
) -> Result<(), String> {
	let bytes = match std::fs::read(root.join(rel)) {
		Ok(bytes) => bytes,
		Err(_) => return Ok(()), // A deletion has no bytes to mirror.
	};
	let file_mode = store.mode_for(session, Path::new(rel))?;
	if store.read(session, Path::new(rel))?.as_deref() != Some(bytes.as_slice()) {
		store.write_blob(session, Path::new(rel), &bytes, &file_mode)?;
		store.note_materialized(session, rel)?;
	}
	Ok(())
}

/// A path the session owns whose disk copy differs from the branch goes onto
/// disk before the native Read sees it stale.
fn materialize_stale(
	store: &crate::store::Store,
	session: &str,
	root: &Path,
	rel: &str,
) -> Result<(), String> {
	if !store
		.owned_paths(session)?
		.iter()
		.any(|(_, owned)| owned == rel)
	{
		return Ok(());
	}
	let branch = store.read(session, Path::new(rel))?;
	let disk = std::fs::read(root.join(rel)).ok();
	if branch == disk {
		return Ok(());
	}
	let result = store.materialize_captured(
		session,
		Some(std::slice::from_ref(&rel.to_owned())),
		false,
		&mut Capture::default(),
	)?;
	for (path, why) in &result.conflicts {
		note(&format!("{path} left on disk as it is: {why}; the branch holds your version, gitfs_materialize with force to take it"));
	}
	Ok(())
}

pub fn run(action: &str) -> i32 {
	run_host(action, false)
}

pub fn run_host(action: &str, codex: bool) -> i32 {
	if let Err(e) = run_inner(action, codex) {
		note(&format!("hook {action}: {e}"));
	}
	0
}

#[cfg(test)]
#[path = "../tests/unit/hooks.rs"]
mod tests;
