//! Bounded file read/write/edit/delete tools. Paths resolve against the canonical
//! session cwd and must stay inside it; mutations are serialized per target
//! and gated on a content-derived read of the current version. In git
//! mode a path inside a repository reads the shared live revision first and
//! publishes changes both to its session branch and the shared live ref;
//! local mode reads and writes the worktree.

use crate::change_record::{
	publication_id, Actor, Evidence, Operation, OverlayCommit, Provider, Storage, Target, Version,
};
use crate::service::{check_cancel, Context, Mode, Read, Service};
use crate::store::Store;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub struct Outcome {
	pub revision: Option<Value>,
	pub content: String,
	pub evidence: Option<Evidence>,
	pub partial: Option<String>,
}

pub enum Input {
	Read {
		path: PathBuf,
		offset: usize,
		limit: usize,
		/// Neither offset nor limit was given: the caller asked for the whole file.
		whole: bool,
		/// The file's exact text, unnumbered: what a postcondition compares.
		raw: bool,
	},
	Write {
		path: PathBuf,
		text: String,
	},
	Edit {
		path: PathBuf,
		/// (old, new, all): one or more exact replacements, applied in order.
		pairs: Vec<(String, String, bool)>,
	},
	Delete {
		path: PathBuf,
	},
}

/// One value from the accepted spellings of a field. Equal repeats are accepted,
/// because a client may echo a schema default; different values are refused.
fn pick_text(field: &str, values: Vec<(&str, Option<String>)>) -> Result<Option<String>, String> {
	let mut chosen: Option<(&str, String)> = None;
	for (name, value) in values {
		let Some(value) = value else { continue };
		match &chosen {
			Some((first, existing)) if *existing != value => {
				return Err(format!(
					"{field} got `{first}` and `{name}` with different values; give one"
				))
			}
			Some(_) => {}
			None => chosen = Some((name, value)),
		}
	}
	Ok(chosen.map(|(_, value)| value))
}

fn pick_flag(field: &str, values: Vec<(&str, Option<bool>)>) -> Result<Option<bool>, String> {
	let mut chosen: Option<(&str, bool)> = None;
	for (name, value) in values {
		let Some(value) = value else { continue };
		match chosen {
			Some((first, existing)) if existing != value => {
				return Err(format!(
					"{field} got `{first}` and `{name}` with different values; give one"
				))
			}
			Some(_) => {}
			None => chosen = Some((name, value)),
		}
	}
	Ok(chosen.map(|(_, value)| value))
}

impl Input {
	pub fn parse(tool: &str, value: Value) -> Result<Self, String> {
		match tool {
			"read" => {
				#[derive(Deserialize)]
				#[serde(deny_unknown_fields)]
				struct Read {
					path: String,
					offset: Option<usize>,
					limit: Option<usize>,
					#[serde(default)]
					raw: bool,
				}
				let read: Read = serde_json::from_value(value).map_err(|e| e.to_string())?;
				let offset = read.offset.unwrap_or(1);
				if offset < 1 {
					return Err("offset must be one-based".into());
				}
				let limit = read.limit.unwrap_or(2000);
				if limit < 1 {
					return Err("limit must be positive".into());
				}
				Ok(Input::Read {
					path: PathBuf::from(read.path),
					offset,
					limit,
					whole: read.offset.is_none() && read.limit.is_none(),
					raw: read.raw,
				})
			}
			"write" => {
				#[derive(Deserialize)]
				#[serde(deny_unknown_fields)]
				struct Write {
					path: String,
					text: Option<String>,
					content: Option<String>,
				}
				let write: Write = serde_json::from_value(value).map_err(|e| e.to_string())?;
				let text = pick_text(
					"write",
					vec![("text", write.text), ("content", write.content)],
				)?
				.ok_or_else(|| "a write needs `text` or `content`".to_owned())?;
				Ok(Input::Write {
					path: PathBuf::from(write.path),
					text,
				})
			}
			"delete" => {
				#[derive(Deserialize)]
				#[serde(deny_unknown_fields)]
				struct Delete {
					path: String,
				}
				let delete: Delete = serde_json::from_value(value).map_err(|e| e.to_string())?;
				Ok(Input::Delete {
					path: PathBuf::from(delete.path),
				})
			}
			"edit" => {
				#[derive(Deserialize)]
				#[serde(deny_unknown_fields)]
				#[allow(non_snake_case)]
				struct Pair {
					old: Option<String>,
					new: Option<String>,
					oldText: Option<String>,
					newText: Option<String>,
					old_string: Option<String>,
					new_string: Option<String>,
					all: Option<bool>,
					replace_all: Option<bool>,
				}
				#[derive(Deserialize)]
				#[serde(deny_unknown_fields)]
				#[allow(non_snake_case)]
				struct Edit {
					path: String,
					old: Option<String>,
					new: Option<String>,
					oldText: Option<String>,
					newText: Option<String>,
					old_string: Option<String>,
					new_string: Option<String>,
					all: Option<bool>,
					replace_all: Option<bool>,
					edits: Option<Vec<Pair>>,
				}
				let edit: Edit = serde_json::from_value(value).map_err(|e| e.to_string())?;
				#[allow(clippy::too_many_arguments)]
				let pair = |old: Option<String>,
				  old_text: Option<String>,
				  old_native: Option<String>,
				  new: Option<String>,
				  new_text: Option<String>,
				  new_native: Option<String>,
				  all: Option<bool>,
				  replace_all: Option<bool>|
				 -> Result<(String, String, bool), String> {
					let old = pick_text(
						"edit",
						vec![
							("old", old),
							("oldText", old_text),
							("old_string", old_native),
						],
					)?
					.ok_or_else(|| {
						"an edit needs `old`/`new`, `oldText`/`newText` or `old_string`/`new_string`"
							.to_owned()
					})?;
					let new = pick_text(
						"edit",
						vec![
							("new", new),
							("newText", new_text),
							("new_string", new_native),
						],
					)?
					.ok_or_else(|| {
						"an edit needs `old`/`new`, `oldText`/`newText` or `old_string`/`new_string`"
							.to_owned()
					})?;
					if old.is_empty() {
						return Err("old must not be empty".to_owned());
					}
					let all = pick_flag("edit", vec![("all", all), ("replace_all", replace_all)])?
						.unwrap_or(false);
					Ok((old, new, all))
				};
				let pairs = match edit.edits {
					Some(list) => {
						if edit.old.is_some()
							|| edit.new.is_some() || edit.oldText.is_some()
							|| edit.newText.is_some()
							|| edit.old_string.is_some()
							|| edit.new_string.is_some()
						{
							return Err("give either `old`/`new` or `edits`, not both".into());
						}
						if list.is_empty() {
							return Err("`edits` must hold at least one replacement".into());
						}
						let mut pairs = Vec::with_capacity(list.len());
						for p in list {
							pairs.push(pair(
								p.old,
								p.oldText,
								p.old_string,
								p.new,
								p.newText,
								p.new_string,
								p.all,
								p.replace_all,
							)?)
						}
						pairs
					}
					None => vec![pair(
						edit.old,
						edit.oldText,
						edit.old_string,
						edit.new,
						edit.newText,
						edit.new_string,
						edit.all,
						edit.replace_all,
					)?],
				};
				Ok(Input::Edit {
					path: PathBuf::from(edit.path),
					pairs,
				})
			}
			_ => Err("unknown tool".into()),
		}
	}
}

pub fn execute(
	service: &Arc<Service>,
	context: &Context,
	input: Input,
	cancel: &AtomicBool,
) -> Result<Outcome, String> {
	match input {
		Input::Read {
			path,
			offset,
			limit,
			whole,
			raw,
		} => read(service, context, &path, offset, limit, whole, raw, cancel),
		Input::Write { path, text } => write(service, context, &path, &text, cancel),
		Input::Edit { path, pairs } => edit(service, context, &path, &pairs, cancel),
		Input::Delete { path } => delete(service, context, &path, cancel),
	}
}

fn version(bytes: &[u8]) -> [u8; 32] {
	Sha256::digest(bytes).into()
}

/// A line as whitespace folding sees it: runs of spaces and tabs become one
/// space and both ends are trimmed. Two lines that fold equal differ only in
/// whitespace.
fn fold_line(line: &str) -> String {
	line.split_whitespace().collect::<Vec<_>>().join(" ")
}

enum Folded {
	None,
	Unique {
		start: usize,
		end: usize,
		line: usize,
	},
	Ambiguous(Vec<usize>),
}

/// Locate `old` in `text` when the only difference is whitespace. The span is
/// byte-exact in `text`, so the replacement leaves every other byte and the
/// file's line endings untouched.
fn folded_span(text: &str, old: &str) -> Folded {
	let wanted = old
		.split_inclusive('\n')
		.map(|line| fold_line(line.trim_end_matches(['\n', '\r'])))
		.collect::<Vec<_>>();
	if wanted.is_empty() {
		return Folded::None;
	}
	let mut lines = Vec::new();
	let mut start = 0;
	for line in text.split_inclusive('\n') {
		let end = start + line.len();
		lines.push((start, end, fold_line(line.trim_end_matches(['\n', '\r']))));
		start = end;
	}
	let mut matches = Vec::new();
	for index in 0..=lines.len().saturating_sub(wanted.len()) {
		if wanted
			.iter()
			.zip(&lines[index..index + wanted.len()])
			.all(|(want, (_, _, have))| want == have)
		{
			matches.push(index);
		}
	}
	match matches.as_slice() {
		[] => Folded::None,
		[index] => Folded::Unique {
			start: lines[*index].0,
			end: lines[*index + wanted.len() - 1].1,
			line: index + 1,
		},
		_ => Folded::Ambiguous(matches.iter().map(|index| index + 1).collect()),
	}
}

fn no_match(at: &str, text: &str, old: &str, rebased: bool) -> String {
	if rebased {
		return format!("{at}file changed since it was read and its old text is gone; reread it");
	}
	let first = old.lines().next().unwrap_or_default();
	let folded = fold_line(first);
	match text
		.lines()
		.enumerate()
		.find(|(_, line)| fold_line(line) == folded)
	{
		Some((index, actual)) => format!(
			"{at}no exact match for old text; line {} matches the first old line after folding whitespace but the rest does not: expected {first:?}, actual {actual:?}; read the file again or make the old text exact",
			index + 1
		),
		None => format!(
			"{at}no match for old text; the first line {first:?} is absent even after folding whitespace; read the file again and use its exact text"
		),
	}
}

fn folded_note(lines: &[usize]) -> String {
	if lines.is_empty() {
		return String::new();
	}
	format!(
		"; whitespace folded at line{} {}",
		if lines.len() == 1 { "" } else { "s" },
		lines
			.iter()
			.map(|line| line.to_string())
			.collect::<Vec<_>>()
			.join(", ")
	)
}

fn unpublished_refusal(session: &str, rel: &Path) -> String {
	format!(
		"session branch changed since its last live publication for {}; inspect with gitfs_diff, then gitfs_catchup to rebase onto trunk or reread the path and retry; session {session}",
		rel.display()
	)
}

/// Resolve `path` against the canonical `cwd`, requiring the result to stay
/// inside it. Symlinks in the existing portion are resolved; a trailing
/// non-existent tail is appended to the canonicalized nearest existing parent.
pub fn resolve(cwd: &Path, path: &Path) -> Result<PathBuf, String> {
	let joined = if path.is_absolute() {
		path.to_path_buf()
	} else {
		cwd.join(path)
	};
	let mut tail: Vec<std::ffi::OsString> = Vec::new();
	let mut ancestor = joined.as_path();
	loop {
		match ancestor.canonicalize() {
			Ok(canon) => {
				let mut resolved = canon;
				for part in tail.iter().rev() {
					resolved.push(part);
				}
				if !resolved.starts_with(cwd) {
					return Err("path escapes the session workspace".into());
				}
				return Ok(resolved);
			}
			Err(_) => {
				let name = ancestor
					.file_name()
					.ok_or_else(|| String::from("path has no resolvable ancestor"))?;
				tail.push(name.to_os_string());
				ancestor = ancestor
					.parent()
					.ok_or_else(|| String::from("path has no resolvable ancestor"))?;
			}
		}
	}
}

/// Search's path validation: same workspace confinement `resolve` enforces,
/// exposed for the compositional search tools' explicit `path` stage.
pub fn resolve_for_search(cwd: &Path, path: &str) -> Result<PathBuf, String> {
	let resolved = resolve(cwd, Path::new(path))?;
	if resolved.starts_with(cwd.join(".kern/memos")) {
		return Err("refusing to search the memo record through tool search; use tool.memo.list or tool.memo.read".into());
	}
	Ok(resolved)
}

fn check_memo_record(cwd: &Path, resolved: &Path) -> Result<(), String> {
	if resolved.starts_with(cwd.join(".kern/memos")) {
		return Err(
			"refusing to write the memo record through tool.write; use tool.memo.write".into(),
		);
	}
	Ok(())
}

/// The session branch a resolved path lives on, when git mode applies.
struct Branch {
	store: Store,
	rel: PathBuf,
}

fn branch(service: &Service, cwd: &Path, resolved: &Path, create: bool) -> Option<Branch> {
	if service.config.mode != Mode::Git {
		return None;
	}
	let store = if create {
		service.store.at(cwd)
	} else {
		service.store.at_readonly(cwd)
	}
	.ok()?;
	let root = store.root().canonicalize().ok()?;
	let rel = resolved.strip_prefix(&root).ok()?.to_path_buf();
	(!rel.as_os_str().is_empty()).then_some(Branch { store, rel })
}

/// The current bytes: shared live revision, then any private session change,
/// then the worktree. `None` when absent from all three.
fn current_bytes(
	branch: Option<&Branch>,
	session: &str,
	resolved: &Path,
) -> Result<Option<Vec<u8>>, String> {
	if let Some(branch) = branch {
		if let Some(change) = branch.store.live_change(&branch.rel)? {
			return Ok(change);
		}
		if let Some(change) = branch.store.session_change(session, &branch.rel)? {
			return Ok(change);
		}
	}
	match fs::read(resolved) {
		Ok(bytes) => Ok(Some(bytes)),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(e) => Err(format!("read failed: {e}")),
	}
}

/// What another tool reads for a workspace path, including peer commits.
#[allow(dead_code)]
pub fn read_current(service: &Service, context: &Context, path: &str) -> Result<Vec<u8>, String> {
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	let resolved = resolve_for_search(&cwd, path)?;
	current_bytes(
		branch(service, &cwd, &resolved, false).as_ref(),
		&context.session,
		&resolved,
	)?
	.ok_or_else(|| format!("{path}: file not found"))
}

/// Publish as one commit on the session branch; the worktree is not touched.
fn commit(
	branch: &Branch,
	context: &Context,
	before: Option<&[u8]>,
	bytes: &[u8],
	operation: Operation,
) -> Result<(Option<Evidence>, String), String> {
	let mut capture = crate::provenance::Capture::from_context(
		&serde_json::json!({"session":context.session,"run":context.run,"call":context.call}),
	)?;
	if branch
		.store
		.unpublished_session_path(&context.session, &branch.rel)?
	{
		return Err(unpublished_refusal(&context.session, &branch.rel));
	}
	let mode = branch.store.mode_for(&context.session, &branch.rel)?;
	let tip = branch.store.write_blob_captured(
		&context.session,
		&branch.rel,
		bytes,
		&mode,
		&mut capture,
		operation,
	)?;
	let live = branch
		.store
		.publish_live(&context.session, &branch.rel, before, Some(bytes), &mode)
		.map_err(|error| {
			format!(
				"session commit {} preserved, but live publication failed: {error}",
				tip.as_deref().unwrap_or("unchanged")
			)
		})?;
	let name = branch.store.branch(&context.session);
	let summary = match tip {
		Some(tip) => format!(
			"committed {} bytes to {} as {tip} on {name}; live at {}; materialize to run it",
			bytes.len(),
			branch.rel.display(),
			live.as_deref().unwrap_or("unchanged")
		),
		None => format!(
			"{} unchanged on {name}; live at {}",
			branch.rel.display(),
			live.as_deref().unwrap_or("unchanged")
		),
	};
	Ok((capture.records.pop(), summary))
}

// The four window fields are `Input::Read`'s own, passed through unchanged.
#[allow(clippy::too_many_arguments)]
fn read(
	service: &Service,
	context: &Context,
	path: &Path,
	offset: usize,
	limit: usize,
	whole: bool,
	raw: bool,
	cancel: &AtomicBool,
) -> Result<Outcome, String> {
	check_cancel(cancel)?;
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	let resolved = resolve(&cwd, path)?;
	check_cancel(cancel)?;
	let branch = branch(service, &cwd, &resolved, false);
	let max_whole = service.config.max_whole_read_lines;
	let mut attempt = 0;
	let (view, head, tip, live_tip, text) = loop {
		attempt += 1;
		let head = branch
			.as_ref()
			.map(|b| b.store.head())
			.transpose()?
			.flatten();
		let tip = branch
			.as_ref()
			.map(|b| b.store.tip(&context.session))
			.transpose()?
			.flatten();
		let live_tip = branch
			.as_ref()
			.map(|branch| branch.store.live_tip())
			.transpose()?
			.flatten();
		let live_change = match &branch {
			Some(branch) => branch.store.live_change(&branch.rel)?,
			None => None,
		};
		let session_change = match &branch {
			Some(branch) => branch.store.session_change(&context.session, &branch.rel)?,
			None => None,
		};
		let (view, change) = if live_change.is_some() {
			("live", live_change)
		} else if session_change.is_some() {
			("session", session_change)
		} else {
			("disk", None)
		};
		let bytes = match change {
			Some(Some(bytes)) => bytes,
			Some(None) => {
				service.mark_read(context, &resolved, Read::Absent)?;
				return Err(format!("file not found: {}", resolved.display()));
			}
			None => {
				let metadata = match fs::metadata(&resolved) {
					Ok(metadata) => metadata,
					Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
						service.mark_read(context, &resolved, Read::Absent)?;
						return Err(format!("file not found: {}", resolved.display()));
					}
					Err(e) => return Err(format!("read failed: {e}")),
				};
				if metadata.len() > service.config.max_file_bytes as u64 {
					return Err(format!(
						"file exceeds {} bytes",
						service.config.max_file_bytes
					));
				}
				fs::read(&resolved).map_err(|e| format!("read failed: {e}"))?
			}
		};
		if bytes.len() > service.config.max_file_bytes {
			return Err(format!(
				"file exceeds {} bytes",
				service.config.max_file_bytes
			));
		}
		let text =
			String::from_utf8(bytes).map_err(|_| "binary or invalid UTF-8 content".to_string())?;
		// A whole-file read of a long file is refused before it establishes
		// freshness, and the caller is pointed at a range, a grep or tool.digest.
		// A raw read answers a runtime postcondition, not an agent's context, so
		// only the byte bound applies to it.
		if whole && !raw {
			let lines = text.lines().count();
			if lines > max_whole {
				return Err(format!(
					"file has {lines} lines, over the {max_whole} allowed for a read without offset or limit; read a range with offset and limit, grep for what you need, or ask tool.digest"
				));
			}
		}
		if let Some(b) = &branch {
			let now_head = b.store.head()?;
			let now_tip = b.store.tip(&context.session)?;
			let now_live = b.store.live_tip()?;
			if now_head != head || now_tip != tip || now_live != live_tip {
				if attempt < 3 {
					continue;
				}
				let (name, before, after) = if now_head != head {
					("trunk HEAD", head.as_deref(), now_head.as_deref())
				} else if now_tip != tip {
					("session tip", tip.as_deref(), now_tip.as_deref())
				} else {
					("live tip", live_tip.as_deref(), now_live.as_deref())
				};
				return Err(format!(
					"revision moved during read on {attempt} consecutive attempts: {name} changed from {} to {}; reread to answer the current view",
					before.unwrap_or("none"),
					after.unwrap_or("none")
				));
			}
		}
		break (view, head, tip, live_tip, text);
	};
	let selected_revision = format!("{:x}", Sha256::digest(text.as_bytes()));
	let disk = match fs::metadata(&resolved) {
		Ok(m) if m.len() <= service.config.max_file_bytes as u64 => match fs::read(&resolved) {
			Ok(bytes) => {
				serde_json::json!({"state":"available","revision":format!("{:x}",Sha256::digest(&bytes))})
			}
			Err(e) => serde_json::json!({"state":"unavailable","error":e.to_string()}),
		},
		Ok(_) => serde_json::json!({"state":"oversized"}),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({"state":"absent"}),
		Err(e) => serde_json::json!({"state":"unavailable","error":e.to_string()}),
	};
	let commit = if view == "live" {
		live_tip
	} else if view == "session" {
		tip
	} else if view == "trunk" {
		head
	} else {
		None
	};
	let revision = revision_block(
		path,
		view,
		&context.session,
		&selected_revision,
		commit,
		disk,
	);
	let version = version(text.as_bytes());
	service.mark_read(context, &resolved, Read::Present(version))?;
	if raw {
		if text.len() > service.config.max_read_bytes {
			return Err(format!(
				"file exceeds {} bytes for a raw read",
				service.config.max_read_bytes
			));
		}
		return Ok(Outcome {
			revision: Some(revision),
			content: text,
			evidence: None,
			partial: None,
		});
	}
	let limit = limit.min(service.config.max_read_lines);
	let lines = text.lines().count();
	if offset > lines.max(1) {
		return Err(format!(
			"offset {offset} is past the end: the file has {lines} lines"
		));
	}
	let content = render(&text, offset, limit, service.config.max_read_bytes);
	Ok(Outcome {
		revision: Some(revision),
		content,
		evidence: None,
		partial: None,
	})
}

fn render(text: &str, offset: usize, limit: usize, max_bytes: usize) -> String {
	let mut out = String::new();
	let mut truncated_lines = false;
	let mut truncated_bytes = false;
	let mut line_no = 0usize;
	for line in text.split_inclusive('\n') {
		line_no += 1;
		if line_no < offset {
			continue;
		}
		if line_no >= offset.saturating_add(limit) {
			truncated_lines = true;
			break;
		}
		let numbered = format!("{line_no}:{line}");
		if out.len() + numbered.len() > max_bytes {
			truncated_bytes = true;
			break;
		}
		out.push_str(&numbered);
	}
	if truncated_lines {
		out.push_str(&format!("... (truncated: more lines beyond {limit})"));
	} else if truncated_bytes {
		out.push_str(&format!("... (truncated at {max_bytes} bytes)"));
	}
	out
}

/// `sibling_revision` admits a disk copy that equals another gitfs session's
/// committed branch content: an edit that is rebasing onto a sibling's work
/// may accept it, while a first whole-file write never adopts it silently.
fn require_disk_agreement(
	branch: Option<&Branch>,
	session: &str,
	path: &Path,
	selected: Option<[u8; 32]>,
	max_bytes: usize,
	sibling_revision: bool,
) -> Result<(), String> {
	let Some(branch) = branch else {
		return Ok(());
	};
	if branch.store.session_change(session, &branch.rel)?.is_some() {
		return Ok(());
	}
	let disk = match fs::metadata(path) {
		Ok(m) if m.len() > max_bytes as u64 => {
			return Err("disk comparison exceeds byte bound".into())
		}
		Ok(_) => Some(version(&fs::read(path).map_err(|e| e.to_string())?)),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
		Err(e) => return Err(e.to_string()),
	};
	// Reads expose local work, but a first write must not silently adopt
	// somebody else's uncommitted tracked edits (including deletions).
	let trunk = branch
		.store
		.trunk_read(&branch.rel)?
		.map(|bytes| version(&bytes));
	// The unchanged worktree is allowed while the selected shared revision
	// has advanced, including a published deletion (selected == None).
	if branch.store.live_change(&branch.rel)?.is_some() && disk == trunk {
		return Ok(());
	}
	if trunk.is_some() && disk != trunk && !sibling_revision {
		return Err("disk differs from committed trunk; reconcile local changes before editing; capture applies only to session-owned paths".into());
	}
	if disk != selected {
		return Err("disk differs from the selected revision; commit disk changes to trunk or restore disk, then reread; capture applies only to session-owned paths".into());
	}
	Ok(())
}

fn write(
	service: &Service,
	context: &Context,
	path: &Path,
	text: &str,
	cancel: &AtomicBool,
) -> Result<Outcome, String> {
	check_cancel(cancel)?;
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	let resolved = resolve(&cwd, path)?;
	check_memo_record(&cwd, &resolved)?;
	check_cancel(cancel)?;
	let last_read = service.last_read(context, &resolved)?;
	let target = service.target(&resolved);
	let _guard = target.lock().unwrap_or_else(|e| e.into_inner());
	check_cancel(cancel)?;
	let branch = branch(service, &cwd, &resolved, true);
	let previous = current_bytes(branch.as_ref(), &context.session, &resolved)
		.map_err(|e| format!("write failed: {e}"))?;
	let current = previous.as_deref().map(version);
	match last_read {
		None => {
			if current.is_some() {
				return Err("unseen write to an existing file; read it first".into());
			}
		}
		Some(Read::Absent) => {
			if current.is_some() {
				return Err("file changed since it was read absent; reread it".into());
			}
		}
		Some(Read::Present(expected)) => match current {
			None => return Err("file disappeared since it was read; reread it".into()),
			Some(actual) if actual != expected => {
				return Err("file changed since it was read; reread it".into())
			}
			Some(_) => {}
		},
	}
	require_disk_agreement(
		branch.as_ref(),
		&context.session,
		&resolved,
		current,
		service.config.max_file_bytes,
		false,
	)?;
	let new_version = version(text.as_bytes());
	if let Some(branch) = &branch {
		check_cancel(cancel)?;
		let (evidence, content) = commit(
			branch,
			context,
			previous.as_deref(),
			text.as_bytes(),
			Operation::Write,
		)?;
		return Ok(Outcome {
			revision: None,
			content,
			evidence,
			partial: mark_committed_read(service, context, &resolved, new_version).err(),
		});
	}
	let evidence = evidence(
		context,
		&cwd,
		&resolved,
		Operation::Write,
		current,
		Some(new_version),
	)?;
	if let Some(parent) = resolved.parent() {
		fs::create_dir_all(parent).map_err(|e| format!("create parents failed: {e}"))?;
	}
	check_cancel(cancel)?;
	let partial = publish(service, &cwd, &resolved, text.as_bytes(), current, cancel)?;
	let partial = combine_partial(
		partial,
		mark_committed_read(service, context, &resolved, new_version).err(),
	);
	Ok(Outcome {
		revision: None,
		content: format!("wrote {} bytes to {}", text.len(), resolved.display()),
		evidence: Some(evidence),
		partial,
	})
}

fn edit(
	service: &Service,
	context: &Context,
	path: &Path,
	pairs: &[(String, String, bool)],
	cancel: &AtomicBool,
) -> Result<Outcome, String> {
	check_cancel(cancel)?;
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	let resolved = resolve(&cwd, path)?;
	check_memo_record(&cwd, &resolved)?;
	check_cancel(cancel)?;
	let last_read = service.last_read(context, &resolved)?;
	let target = service.target(&resolved);
	let _guard = target.lock().unwrap_or_else(|e| e.into_inner());
	check_cancel(cancel)?;
	let expected = match last_read {
		// why: a read through another session or a built-in tool leaves this
		// session blind; the message names the session so the caller rereads
		// through the right tool event instead of retrying the edit.
		None => {
			return Err(format!(
				"unseen edit; read the file first through session `{}` (a read through another session or tool does not count)",
				context.session
			))
		}
		Some(Read::Absent) => return Err("file is absent; read it first".into()),
		Some(Read::Present(expected)) => expected,
	};
	let branch = branch(service, &cwd, &resolved, true);
	let bytes = current_bytes(branch.as_ref(), &context.session, &resolved)
		.map_err(|e| format!("edit failed: {e}"))?
		.ok_or("file disappeared since it was read; reread it")?;
	// A sibling agent on the same worktree can move a file this session read:
	// its change is committed on that agent's session branch (or already on
	// trunk) and materialize put it on the shared disk before trunk moved. The
	// replacement decides whether that is safe — when its `old` text still
	// matches uniquely, the edit rebases onto the newer text instead of making
	// the caller reread and race the same file again. Bytes that are neither
	// trunk nor a gitfs session's revision are an unknown hand edit, and the
	// edit still refuses.
	let rebased = if version(&bytes) != expected {
		let known = match &branch {
			Some(branch) => branch
				.store
				.known_revision(&context.session, &branch.rel, &bytes)?,
			None => false,
		};
		if !known {
			return Err("file changed since it was read; reread it".into());
		}
		require_disk_agreement(
			branch.as_ref(),
			&context.session,
			&resolved,
			Some(version(&bytes)),
			service.config.max_file_bytes,
			true,
		)?;
		true
	} else {
		require_disk_agreement(
			branch.as_ref(),
			&context.session,
			&resolved,
			Some(expected),
			service.config.max_file_bytes,
			false,
		)?;
		false
	};
	let before = bytes.clone();
	let text =
		String::from_utf8(bytes).map_err(|_| "binary or invalid UTF-8 content".to_string())?;
	let mut replaced = text;
	let mut folded_lines = Vec::new();
	for (index, (old, new, all)) in pairs.iter().enumerate() {
		let at = if pairs.len() == 1 {
			String::new()
		} else {
			format!("edit {}: ", index + 1)
		};
		let count = replaced.matches(old).count();
		if count == 0 {
			match folded_span(&replaced, old) {
				Folded::Unique { start, end, line } => {
					let mut next = String::with_capacity(replaced.len() + new.len());
					next.push_str(&replaced[..start]);
					next.push_str(new);
					next.push_str(&replaced[end..]);
					replaced = next;
					folded_lines.push(line);
					continue;
				}
				Folded::Ambiguous(lines) => {
					return Err(format!(
						"{at}no exact match for old text; folding whitespace matches {} places (lines {}); use all=true or a more specific old text",
						lines.len(),
						lines
							.iter()
							.map(|line| line.to_string())
							.collect::<Vec<_>>()
							.join(", ")
					));
				}
				Folded::None => return Err(no_match(&at, &replaced, old, rebased)),
			}
		}
		if !*all && count > 1 {
			return Err(if rebased {
				format!(
					"{at}file changed since it was read and its old text is no longer unique ({count} matches); reread it"
				)
			} else {
				format!(
					"{at}ambiguous edit: {count} matches; use all=true or a more specific old text"
				)
			});
		}
		replaced = replaced.replace(old.as_str(), new.as_str());
	}
	let new_version = version(replaced.as_bytes());
	if let Some(branch) = &branch {
		check_cancel(cancel)?;
		let (evidence, mut content) = commit(
			branch,
			context,
			Some(&before),
			replaced.as_bytes(),
			Operation::Edit,
		)?;
		content.push_str(&folded_note(&folded_lines));
		return Ok(Outcome {
			revision: None,
			content,
			evidence,
			partial: mark_committed_read(service, context, &resolved, new_version).err(),
		});
	}
	let evidence = evidence(
		context,
		&cwd,
		&resolved,
		Operation::Edit,
		Some(expected),
		Some(new_version),
	)?;
	let partial = publish(
		service,
		&cwd,
		&resolved,
		replaced.as_bytes(),
		Some(expected),
		cancel,
	)?;
	let partial = combine_partial(
		partial,
		mark_committed_read(service, context, &resolved, new_version).err(),
	);
	Ok(Outcome {
		revision: None,
		content: format!(
			"edited {}{}",
			resolved.display(),
			folded_note(&folded_lines)
		),
		evidence: Some(evidence),
		partial,
	})
}

/// The caller holds the service's target lock from read validation
/// through publication. This recheck catches external edits during preparation;
/// ordinary rename is not a compare-and-swap against arbitrary OS writers.
fn publish(
	service: &Service,
	cwd: &Path,
	path: &Path,
	bytes: &[u8],
	expected: Option<[u8; 32]>,
	cancel: &AtomicBool,
) -> Result<Option<String>, String> {
	let permissions = if expected.is_some() {
		Some(
			fs::metadata(path)
				.map_err(|e| format!("metadata failed: {e}"))?
				.permissions(),
		)
	} else {
		None
	};
	let temp = temp_sibling(path)?;
	// Do not truncate a pre-existing temporary path, or remove it on open failure.
	let mut file = fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(&temp)
		.map_err(|e| format!("prepare failed: {e}"))?;
	let result = (|| -> Result<Option<String>, String> {
		file.write_all(bytes)
			.map_err(|e| format!("prepare failed: {e}"))?;
		if let Some(permissions) = permissions {
			file.set_permissions(permissions)
				.map_err(|e| format!("permissions failed: {e}"))?;
		}
		file.sync_all().map_err(|e| format!("sync failed: {e}"))?;
		service.before_commit(path);
		check_cancel(cancel)?;
		if resolve(cwd, path)? != path || resolve(cwd, &temp)? != temp {
			return Err("conflict: path changed during preparation; reread it".into());
		}
		check_memo_record(cwd, path)?;
		let actual = match fs::read(path) {
			Ok(bytes) => Some(version(&bytes)),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
			Err(e) => return Err(format!("publication check failed: {e}")),
		};
		if actual != expected {
			return Err("conflict: file changed during preparation; reread it".into());
		}
		check_cancel(cancel)?;
		if expected.is_some() {
			fs::rename(&temp, path).map_err(|e| format!("publish failed: {e}"))?;
		} else {
			// A link publishes all prepared bytes at once and cannot replace a winner.
			fs::hard_link(&temp, path)
				.map_err(|e| format!("create conflict or failure: {e}; reread it"))?;
			service.after_publish(path);
			if let Err(e) = fs::remove_file(&temp) {
				return Ok(Some(format!("partial_success: file already changed: {}; temp cleanup failed: {e}. Do not automatically retry.", path.display())));
			}
		}
		Ok(None)
	})();
	if result.is_err() {
		let _ = fs::remove_file(&temp);
	}
	result
}

fn delete(
	service: &Service,
	context: &Context,
	path: &Path,
	cancel: &AtomicBool,
) -> Result<Outcome, String> {
	check_cancel(cancel)?;
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	let requested = cwd.join(path);
	if fs::symlink_metadata(&requested).is_ok_and(|m| m.file_type().is_symlink()) {
		return Err("delete refuses symbolic links; name a regular file".into());
	}
	let resolved = resolve(&cwd, path)?;
	check_memo_record(&cwd, &resolved)?;
	let last_read = service.last_read(context, &resolved)?;
	let target = service.target(&resolved);
	let _guard = target.lock().unwrap_or_else(|e| e.into_inner());
	let branch = branch(service, &cwd, &resolved, true);
	if service.config.mode == Mode::Git && branch.is_none() {
		return Err("cannot open the Git session store for deletion".into());
	}
	let previous = current_bytes(branch.as_ref(), &context.session, &resolved)?;
	let current = previous.as_deref().map(version);
	let expected = match (last_read, current) {
		(Some(Read::Absent), None) => {
			return Ok(Outcome {
				revision: None,
				content: format!("{} already absent", path.display()),
				evidence: None,
				partial: None,
			})
		}
		(None, _) => return Err("unseen delete; read the file first".into()),
		(Some(Read::Absent), _) => {
			return Err("file changed since it was read absent; reread it".into())
		}
		(Some(Read::Present(_)), None) => {
			return Err("file disappeared since it was read; reread it".into())
		}
		(Some(Read::Present(expected)), Some(actual)) if expected == actual => expected,
		_ => return Err("file changed since it was read; reread it".into()),
	};
	require_disk_agreement(
		branch.as_ref(),
		&context.session,
		&resolved,
		Some(expected),
		service.config.max_file_bytes,
		false,
	)?;
	let (evidence, content) = if let Some(branch) = &branch {
		let selected = branch.store.selected_blob(&context.session, &branch.rel)?;
		if selected.is_none() && !matches!(branch.store.live_change(&branch.rel)?, Some(Some(_))) {
			return Err("file is not tracked on the session, trunk or live revision; use local mode to delete an untracked file".into());
		}
		if branch
			.store
			.unpublished_session_path(&context.session, &branch.rel)?
		{
			return Err(unpublished_refusal(&context.session, &branch.rel));
		}
		let mut capture = crate::provenance::Capture::from_context(
			&serde_json::json!({"session":context.session,"run":context.run,"call":context.call}),
		)?;
		service.before_commit(&resolved);
		check_cancel(cancel)?;
		let tip = if selected.is_some() {
			branch.store.delete_blob_checked(
				&context.session,
				&branch.rel,
				expected,
				&mut capture,
			)?
		} else {
			branch.store.delete_blob_captured(
				&context.session,
				&branch.rel,
				&mut capture,
				Operation::Delete,
			)?
		};
		let live = branch
			.store
			.publish_live(
				&context.session,
				&branch.rel,
				previous.as_deref(),
				None,
				"100644",
			)
			.map_err(|error| {
				format!(
					"session deletion {} preserved, but live publication failed: {error}",
					tip.as_deref().unwrap_or("unchanged")
				)
			})?;
		(
			capture.records.pop(),
			format!(
				"deleted {} on {} at {}; live at {}; materialize to remove it from disk",
				branch.rel.display(),
				branch.store.branch(&context.session),
				tip.as_deref().unwrap_or("unchanged"),
				live.as_deref().unwrap_or("unchanged")
			),
		)
	} else {
		let evidence = evidence(
			context,
			&cwd,
			&resolved,
			Operation::Delete,
			Some(expected),
			None,
		)?;
		service.before_commit(&resolved);
		check_cancel(cancel)?;
		if resolve(&cwd, &resolved)? != resolved
			|| version(&fs::read(&resolved).map_err(|e| format!("deletion check failed: {e}"))?)
				!= expected
		{
			return Err("conflict: file changed during preparation; reread it".into());
		}
		check_memo_record(&cwd, &resolved)?;
		fs::remove_file(&resolved).map_err(|e| format!("delete failed: {e}"))?;
		(Some(evidence), format!("deleted {}", resolved.display()))
	};
	let partial = service.mark_read(context, &resolved, Read::Absent).err().map(|e| format!("partial_success: file already deleted: {}; read record failed: {e}. Do not automatically retry.", resolved.display()));
	Ok(Outcome {
		revision: None,
		content,
		evidence,
		partial,
	})
}

fn evidence(
	context: &Context,
	cwd: &Path,
	path: &Path,
	operation: Operation,
	before: Option<[u8; 32]>,
	after: Option<[u8; 32]>,
) -> Result<Evidence, String> {
	let hex = |bytes: [u8; 32]| bytes.iter().map(|v| format!("{v:02x}")).collect::<String>();
	let evidence = Evidence {
		v: 1,
		publication_id: publication_id()?,
		provider: Provider::Fs,
		operation,
		actor: Actor {
			session: context.session.clone(),
			run: Some(context.run.clone()),
			call: Some(context.call.clone()),
		},
		target: Target {
			workspace: cwd.to_str().ok_or("workspace is not UTF-8")?.into(),
			path: path.to_str().ok_or("target is not UTF-8")?.into(),
			storage: Storage::Direct,
			overlay_ref: None,
		},
		before: before.map_or(Version::Absent {}, |value| Version::Sha256 {
			value: hex(value),
		}),
		after: after.map_or(Version::Absent {}, |value| Version::Sha256 {
			value: hex(value),
		}),
		overlay_commit: OverlayCommit {
			before: None,
			after: None,
		},
	};
	evidence.validate(&context.session)?;
	Ok(evidence)
}
fn combine_partial(first: Option<String>, second: Option<String>) -> Option<String> {
	match (first, second) {
		(Some(a), Some(b)) => Some(format!("{a} {b}")),
		(Some(a), None) | (None, Some(a)) => Some(a),
		(None, None) => None,
	}
}

fn mark_committed_read(
	service: &Service,
	context: &Context,
	path: &Path,
	revision: [u8; 32],
) -> Result<(), String> {
	service.mark_read(context, path, Read::Present(revision)).map_err(|e| {
		format!("partial_success: file already changed: {}; read record failed: {e}. Do not automatically retry.", path.display())
	})
}

/// The revision metadata of a read. A disk copy equal to the selected revision
/// carries no second hash (`matches_disk` says so); absent facts are omitted
/// rather than sent as null; divergence keeps the disk hash and the reconcile hint.
fn revision_block(
	path: impl serde::Serialize,
	view: &str,
	session: &str,
	selected: &str,
	commit: Option<impl serde::Serialize>,
	disk: serde_json::Value,
) -> serde_json::Value {
	let matches_disk = disk["revision"].as_str().map(|r| r == selected);
	let mut revision = serde_json::json!({"path":path,"view":view,"revision":selected,"revision_kind":"source_bytes","matches_disk":matches_disk});
	if view == "session" {
		revision["session"] = session.into();
	}
	if let Some(commit) = commit {
		revision["commit"] = serde_json::json!(commit);
	}
	revision["disk"] = if matches_disk == Some(true) {
		serde_json::json!({"state":"available"})
	} else {
		disk
	};
	if matches_disk == Some(false) {
		revision["reconciliation"] = "reconcile unowned disk edits with trunk before editing; capture only session-owned paths; review materialization before building".into();
	}
	revision
}

#[cfg(test)]
mod revision_block_tests {
	use super::revision_block;
	use serde_json::json;

	fn old(
		path: &str,
		view: &str,
		session: &str,
		rev: &str,
		commit: Option<&str>,
		disk: serde_json::Value,
	) -> serde_json::Value {
		let m = disk["revision"].as_str().map(|r| r == rev);
		json!({"path":path,"view":view,"revision":rev,"revision_kind":"source_bytes","session":session,"commit":commit,"disk":disk,"matches_disk":m,"reconciliation":if m == Some(false) { Some("reconcile unowned disk edits with trunk before editing; capture only session-owned paths; review materialization before building") } else { None }})
	}

	const H: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
	const D: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

	#[test]
	fn matching_disk_drops_the_second_hash_and_keeps_identity() {
		let disk = json!({"state":"available","revision":H});
		let r = revision_block("a.rs", "session", "main", H, Some("c1"), disk.clone());
		assert_eq!(
			(r["revision"].as_str(), r["matches_disk"].as_bool()),
			(Some(H), Some(true))
		);
		assert_eq!(
			(
				r["view"].as_str(),
				r["session"].as_str(),
				r["commit"].as_str(),
				r["path"].as_str()
			),
			(Some("session"), Some("main"), Some("c1"), Some("a.rs"))
		);
		assert_eq!(r["disk"], json!({"state":"available"}));
		assert!(r.get("reconciliation").is_none());
		let (before, after) = (
			old("a.rs", "session", "main", H, Some("c1"), disk)
				.to_string()
				.len(),
			r.to_string().len(),
		);
		assert!(after < before, "{after} !< {before}");
		eprintln!("revision bytes matching: before {before} after {after}");
	}

	#[test]
	fn divergent_disk_keeps_hash_and_reconciliation() {
		let disk = json!({"state":"available","revision":D});
		let r = revision_block("a.rs", "trunk", "main", H, Some("c1"), disk.clone());
		assert_eq!(
			(r["matches_disk"].as_bool(), r["disk"].clone()),
			(Some(false), disk)
		);
		assert!(r["reconciliation"].as_str().unwrap().contains("reconcile"));
		assert_eq!(r["commit"], "c1");
		assert!(r.get("session").is_none());
	}

	#[test]
	fn absent_and_unavailable_disk_stay_explicit() {
		for disk in [
			json!({"state":"absent"}),
			json!({"state":"oversized"}),
			json!({"state":"unavailable","error":"denied"}),
		] {
			let r = revision_block("a.rs", "disk", "main", H, None::<&str>, disk.clone());
			assert_eq!(
				(r["disk"].clone(), r["matches_disk"].clone()),
				(disk, json!(null))
			);
			assert!(r.get("reconciliation").is_none() && r.get("commit").is_none());
		}
	}
}

#[cfg(test)]
#[path = "../tests/unit/git_mode.rs"]
mod git_mode_tests;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_sibling(path: &Path) -> Result<PathBuf, String> {
	let dir = path
		.parent()
		.ok_or_else(|| "no parent directory".to_string())?;
	let name = path.file_name().ok_or_else(|| "no file name".to_string())?;
	let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
	Ok(dir.join(format!(
		".{}.{}.{}.tmp",
		name.to_string_lossy(),
		std::process::id(),
		n
	)))
}

#[cfg(test)]
mod edit_input_tests {
	use super::Input;
	use serde_json::json;

	fn parse(value: serde_json::Value) -> Result<Input, String> {
		Input::parse("edit", value)
	}

	#[test]
	fn a_flat_old_new_pair_still_parses_as_one_pair() {
		let Input::Edit { pairs, .. } =
			parse(json!({"path": "a.rs", "old": "x", "new": "y", "all": true})).unwrap()
		else {
			panic!("an edit")
		};
		assert_eq!(pairs, vec![("x".into(), "y".into(), true)]);
	}

	#[test]
	fn an_edits_array_parses_in_both_key_styles() {
		let Input::Edit { pairs, .. } = parse(json!({"path": "a.rs", "edits": [
			{"old": "x", "new": "y"},
			{"oldText": "p", "newText": "q", "all": true}
		]}))
		.unwrap() else {
			panic!("an edit")
		};
		assert_eq!(
			pairs,
			vec![
				("x".into(), "y".into(), false),
				("p".into(), "q".into(), true)
			]
		);
	}

	#[test]
	fn half_entries_both_forms_and_empty_edits_are_refused() {
		for bad in [
			json!({"path": "a.rs", "edits": [{"old": "x"}]}),
			json!({"path": "a.rs", "edits": [{"oldText": "x"}]}),
			json!({"path": "a.rs", "edits": []}),
			json!({"path": "a.rs", "old": "x", "new": "y", "edits": [{"old": "a", "new": "b"}]}),
			json!({"path": "a.rs"}),
			json!({"path": "a.rs", "old": "", "new": "y"}),
			json!({"path": "a.rs", "old": "x"}),
			json!({"path": "a.rs", "new": "y"}),
		] {
			assert!(parse(bad.clone()).is_err(), "{}", bad);
		}
	}

	#[test]
	fn a_raw_read_parses_and_defaults_to_numbered() {
		let raw = |value| match Input::parse("read", value).unwrap() {
			Input::Read { raw, .. } => raw,
			_ => panic!("a read"),
		};
		assert!(raw(json!({"path": "a.rs", "raw": true})));
		assert!(!raw(json!({"path": "a.rs"})));
	}

	#[test]
	fn native_argument_names_parse_as_aliases() {
		let Input::Edit { pairs, .. } = parse(json!({
			"path": "a.rs", "old_string": "x", "new_string": "y", "replace_all": true
		}))
		.unwrap() else {
			panic!("an edit")
		};
		assert_eq!(pairs, vec![("x".into(), "y".into(), true)]);
		let Input::Edit { pairs, .. } = parse(json!({"path": "a.rs", "edits": [
			{"old_string": "x", "new_string": "y"}
		]}))
		.unwrap() else {
			panic!("an edit")
		};
		assert_eq!(pairs, vec![("x".into(), "y".into(), false)]);
		match Input::parse("write", json!({"path": "a.rs", "content": "body"})).unwrap() {
			Input::Write { text, .. } => assert_eq!(text, "body"),
			_ => panic!("a write"),
		}
		match Input::parse(
			"write",
			json!({"path": "a.rs", "text": "body", "content": "body"}),
		)
		.unwrap()
		{
			Input::Write { text, .. } => assert_eq!(text, "body"),
			_ => panic!("a write"),
		}
	}

	#[test]
	fn conflicting_alias_values_are_refused_by_name() {
		for bad in [
			json!({"path": "a.rs", "old": "x", "old_string": "y", "new": "z"}),
			json!({"path": "a.rs", "old": "x", "new": "y", "all": true, "replace_all": false}),
			json!({"path": "a.rs", "edits": [{"old": "x", "old_string": "y", "new": "z"}]}),
		] {
			let error = match parse(bad.clone()) {
				Ok(_) => panic!("accepted {bad}"),
				Err(error) => error,
			};
			assert!(error.contains("different values"), "{bad}: {error}");
		}
		let error = match Input::parse(
			"write",
			json!({"path": "a.rs", "text": "a", "content": "b"}),
		) {
			Ok(_) => panic!("accepted conflicting write fields"),
			Err(error) => error,
		};
		assert!(error.contains("different values"), "{error}");
	}
}

#[cfg(test)]
mod edit_fold_tests {
	use super::{folded_span, Folded};

	#[test]
	fn trailing_whitespace_folds_to_one_span() {
		let text = "one\nlet x = 1;  \ntwo\n";
		match folded_span(text, "let x = 1;\n") {
			Folded::Unique { start, end, line } => {
				assert_eq!(line, 2);
				assert_eq!(&text[start..end], "let x = 1;  \n");
			}
			_ => panic!("one folded match"),
		}
	}

	#[test]
	fn tab_and_space_runs_fold_and_two_windows_refuse() {
		match folded_span("a\t b\nc  d\n", "a b\n") {
			Folded::Unique { line, .. } => assert_eq!(line, 1),
			_ => panic!("one folded match"),
		}
		match folded_span("a b\na  b\n", "a\tb\n") {
			Folded::Ambiguous(lines) => assert_eq!(lines, vec![1, 2]),
			_ => panic!("two folded matches"),
		}
	}
}
