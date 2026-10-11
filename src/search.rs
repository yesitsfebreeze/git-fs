//! Composable typed search over the workspace, following
//! [[decision/composable-agent-search]]: stages chain left to right over a
//! typed result set (path set or match set), a `ref` reuses a prior bounded
//! result within the same run, and every backend call is an `rg` argument
//! array with config isolation, caps, and reaping on deadline or cancel.

use crate::files;
use crate::service::{check_cancel, Context, Mode, Service};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Stdio;

use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// One stage of a chain. Parsed with `deny_unknown_fields`; compatibility is
/// checked against the previous result kind before execution.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Stage {
	Files {
		pattern: Option<String>,
	},
	Dirs {
		pattern: Option<String>,
	},
	Grep {
		pattern: String,
		path: Option<String>,
		glob: Option<String>,
	},
	Matches {
		pattern: String,
	},
	#[serde(rename = "files_of")]
	FilesOf {
		glob: Option<String>,
	},
}

/// Typed result set: workspace-relative paths, or line-addressed matches.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultSet {
	Paths(Vec<String>),
	Matches(Vec<MatchItem>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchItem {
	pub path: String,
	pub line: u64,
	pub text: String,
}

impl ResultSet {
	pub fn kind(&self) -> &'static str {
		match self {
			ResultSet::Paths(_) => "paths",
			ResultSet::Matches(_) => "matches",
		}
	}

	pub fn len(&self) -> usize {
		match self {
			ResultSet::Paths(items) => items.len(),
			ResultSet::Matches(items) => items.len(),
		}
	}

	pub fn to_value(&self) -> Value {
		match self {
			ResultSet::Paths(items) => json!(items),
			ResultSet::Matches(items) => json!(items
				.iter()
				.map(|m| json!({"path": m.path, "line": m.line, "text": m.text}))
				.collect::<Vec<_>>()),
		}
	}
}

pub struct ChainOutcome {
	pub result: ResultSet,
	/// Index of the final chain result in the run's store.
	pub stored: usize,
	pub truncated: bool,
	pub next_cursor: Option<String>,
	pub revision: String,
	pub total: usize,
}

/// Parse and execute a chain. `ref` (when given) reuses a prior result of the
/// same run instead of enumerating from scratch. Empty stages yield an empty
/// set; incompatible stages are rejected before execution.
pub async fn run_chain(
	service: &Arc<Service>,
	context: &Context,
	cancel: &std::sync::atomic::AtomicBool,
	input: Value,
) -> Result<ChainOutcome, String> {
	#[derive(Deserialize)]
	#[serde(deny_unknown_fields)]
	struct SearchInput {
		r#ref: Option<usize>,
		cursor: Option<String>,
		#[serde(default)]
		stages: Vec<Stage>,
	}
	check_cancel(cancel)?;
	let input: SearchInput = serde_json::from_value(input).map_err(|e| e.to_string())?;
	if let Some(cursor) = input.cursor {
		if input.r#ref.is_some() || !input.stages.is_empty() {
			return Err("invalid_cursor: cursor cannot be combined with ref or stages".into());
		}
		let fields: Vec<_> = cursor.split(':').collect();
		if fields.len() != 4 || fields[0] != "v1" {
			return Err("invalid_cursor".into());
		}
		let stored = fields[1].parse::<usize>().map_err(|_| "invalid_cursor")?;
		let offset = fields[2].parse::<usize>().map_err(|_| "invalid_cursor")?;
		let result = service
			.chain_result(context, stored)?
			.ok_or("invalid_or_expired_cursor: unknown results")?;
		let revision = results_revision(service, context, stored, &result);
		if offset == 0 || offset >= result.len() || cursor != cursor_for(stored, offset, &revision)
		{
			return Err("invalid_cursor: results or offset mismatch".into());
		}
		return page(service, result, stored, offset, revision);
	}
	let mut current = match input.r#ref {
		Some(index) => service
			.chain_result(context, index)?
			.ok_or_else(|| format!("unknown ref: {index}"))?,
		None => ResultSet::Paths(Vec::new()),
	};
	for (index, stage) in input.stages.iter().enumerate() {
		check_cancel(cancel)?;
		let start = index == 0 && input.r#ref.is_none();
		check_compatible(stage, &current, start)?;
		current = execute_stage(service, context, cancel, stage, &current, start).await?;
	}
	check_cancel(cancel)?;
	// Validate item size before retaining results that could never be paged.
	for item in current.to_value().as_array().expect("result array") {
		if item.to_string().len() + 2 > service.config.max_search_bytes {
			return Err(
				"capacity_exceeded: one search item exceeds the page byte cap; narrow the query"
					.into(),
			);
		}
	}
	let stored = match (input.stages.is_empty(), input.r#ref) {
		(true, Some(index)) => index,
		_ => service.store_chain_result(context, current.clone())?,
	};
	let revision = results_revision(service, context, stored, &current);
	page(service, current, stored, 0, revision)
}

fn results_revision(
	service: &Service,
	context: &Context,
	stored: usize,
	result: &ResultSet,
) -> String {
	format!(
		"{:x}",
		Sha256::digest(
			json!([
				service.search_epoch,
				context.session,
				context.run,
				context.cwd,
				stored,
				result.to_value()
			])
			.to_string()
			.as_bytes()
		)
	)
}

fn cursor_for(stored: usize, offset: usize, revision: &str) -> String {
	let digest = format!(
		"{:x}",
		Sha256::digest(format!("{revision}:{offset}").as_bytes())
	);
	format!("v1:{stored}:{offset}:{digest}")
}

fn page(
	service: &Service,
	result: ResultSet,
	stored: usize,
	offset: usize,
	revision: String,
) -> Result<ChainOutcome, String> {
	let total = result.len();
	let values = result.to_value();
	let mut bytes = 2;
	let mut end = offset;
	for value in values
		.as_array()
		.expect("result array")
		.iter()
		.skip(offset)
		.take(service.config.max_search_items)
	{
		let size = value.to_string().len() + usize::from(end > offset);
		if bytes + size > service.config.max_search_bytes {
			break;
		}
		bytes += size;
		end += 1;
	}
	if end == offset && offset < total {
		return Err("capacity_exceeded: search item exceeds page byte cap".into());
	}
	let result = match result {
		ResultSet::Paths(items) => ResultSet::Paths(items[offset..end].to_vec()),
		ResultSet::Matches(items) => ResultSet::Matches(items[offset..end].to_vec()),
	};
	Ok(ChainOutcome {
		result,
		stored,
		truncated: end < total,
		next_cursor: (end < total).then(|| cursor_for(stored, end, &revision)),
		revision,
		total,
	})
}

fn stage_name(stage: &Stage) -> &'static str {
	match stage {
		Stage::Files { .. } => "files",
		Stage::Dirs { .. } => "dirs",
		Stage::Grep { .. } => "grep",
		Stage::Matches { .. } => "matches",
		Stage::FilesOf { .. } => "files_of",
	}
}

/// Convenience tools over the same engine: `glob` is a one-stage files chain,
/// `grep` a one-stage grep chain. Their inputs reuse the stage shapes so
/// describe stays truthful.
pub fn as_chain(tool: &str, input: Value) -> Result<Value, String> {
	let stages = match tool {
		"glob" => {
			#[derive(Deserialize)]
			#[serde(deny_unknown_fields)]
			struct Glob {
				pattern: Option<String>,
			}
			let glob: Glob = serde_json::from_value(input).map_err(|e| e.to_string())?;
			json!([{ "kind": "files", "pattern": glob.pattern }])
		}
		"grep" => {
			#[derive(Deserialize)]
			#[serde(deny_unknown_fields)]
			struct Grep {
				pattern: String,
				path: Option<String>,
				glob: Option<String>,
			}
			let grep: Grep = serde_json::from_value(input).map_err(|e| e.to_string())?;
			json!([{ "kind": "grep", "pattern": grep.pattern, "path": grep.path, "glob": grep.glob }])
		}
		"search" => return Ok(input),
		_ => return Err("unknown tool".into()),
	};
	Ok(json!({ "stages": stages }))
}

/// Compatibility: a stage must fit the kind it receives. `matches` and
/// `files_of` need match items; everything else needs a path set or an empty
/// start. Grepping match items directly is refused: project with `files_of`
/// first, which is the explicit whole-file re-search step.
fn check_compatible(stage: &Stage, current: &ResultSet, start: bool) -> Result<(), String> {
	let name = stage_name(stage);
	if start {
		return match name {
			"files" | "dirs" | "grep" => Ok(()),
			_ => Err(format!("stage {name} cannot start a chain")),
		};
	}
	match (name, current.kind()) {
		("files", "paths") | ("dirs", "paths") | ("grep", "paths") => Ok(()),
		("matches", "matches") | ("files_of", "matches") => Ok(()),
		("grep", "matches") => Err(
			"incompatible stage: grep does not accept match-set results; project matches with files_of first".into(),
		),
		_ => Err(format!(
			"incompatible stage: {name} does not accept {} results",
			if current.kind() == "paths" {
				"path-set"
			} else {
				"match-set"
			}
		)),
	}
}

pub(crate) async fn execute_stage(
	service: &Arc<Service>,
	context: &Context,
	cancel: &std::sync::atomic::AtomicBool,
	stage: &Stage,
	current: &ResultSet,
	start: bool,
) -> Result<ResultSet, String> {
	let cwd = Path::new(&context.cwd)
		.canonicalize()
		.map_err(|e| format!("cwd unavailable: {e}"))?;
	match stage {
		Stage::Files { pattern } => match current {
			ResultSet::Paths(paths) => {
				// An empty start enumerates; a non-empty set narrows.
				if start {
					let items =
						enumerate(service, context, &cwd, cancel, pattern.as_deref()).await?;
					return Ok(ResultSet::Paths(items));
				}
				let narrowed: Vec<String> = paths
					.iter()
					.filter(|p| matches_glob(p, pattern.as_deref()))
					.cloned()
					.collect();
				Ok(ResultSet::Paths(narrowed))
			}
			ResultSet::Matches(_) => Err("incompatible stage".into()),
		},
		Stage::Dirs { pattern } => match current {
			ResultSet::Paths(paths) => {
				let initial;
				let paths = if start {
					initial = enumerate(service, context, &cwd, cancel, None).await?;
					&initial
				} else {
					paths
				};
				let mut dirs: Vec<String> = paths
					.iter()
					.filter(|p| matches_glob(p, pattern.as_deref()))
					.map(|p| {
						Path::new(p)
							.parent()
							.map(|d| d.to_string_lossy().into_owned())
							.unwrap_or_else(|| p.clone())
					})
					.collect();
				dirs.sort();
				dirs.dedup();
				Ok(ResultSet::Paths(dirs))
			}
			ResultSet::Matches(_) => Err("incompatible stage".into()),
		},
		Stage::Grep {
			pattern,
			path,
			glob,
		} => {
			let paths =
				match (current, path) {
					// An explicit path is resolved and workspace-validated.
					(_, Some(p)) => {
						let resolved = files::resolve_for_search(&cwd, p)?;
						vec![resolved.to_string_lossy().into_owned()]
					}
					(ResultSet::Paths(paths), None) => paths.clone(),
					(ResultSet::Matches(_), None) => return Err(
						"incompatible stage: project matches with files_of before grepping them"
							.into(),
					),
				};
			if !start && paths.is_empty() {
				return Ok(ResultSet::Matches(Vec::new()));
			}
			let items = grep_paths(
				service,
				context,
				&cwd,
				cancel,
				pattern,
				&paths,
				glob.as_deref(),
			)
			.await?;
			Ok(ResultSet::Matches(items))
		}
		Stage::Matches { pattern } => match current {
			ResultSet::Matches(items) => {
				let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
				let narrowed: Vec<MatchItem> = items
					.iter()
					.filter(|m| re.is_match(&m.text))
					.cloned()
					.collect();
				Ok(ResultSet::Matches(narrowed))
			}
			ResultSet::Paths(_) => Err("incompatible stage".into()),
		},
		Stage::FilesOf { glob } => match current {
			ResultSet::Matches(items) => {
				let mut files: Vec<String> = items
					.iter()
					.filter(|m| matches_glob(&m.path, glob.as_deref()))
					.map(|m| m.path.clone())
					.collect();
				files.sort();
				files.dedup();
				Ok(ResultSet::Paths(files))
			}
			ResultSet::Paths(_) => Err("incompatible stage".into()),
		},
	}
}

/// rg's documented glob matching, reused through `globset` for the narrowing
/// stages; `rg --files --glob` still does its own matching for enumeration.
fn matches_glob(path: &str, pattern: Option<&str>) -> bool {
	let Some(pattern) = pattern else {
		return true;
	};
	globset::GlobBuilder::new(pattern)
		.literal_separator(true)
		.build()
		.ok()
		.map(|g| {
			globset::GlobSetBuilder::new()
				.add(g)
				.build()
				.map(|s| s.is_match(path))
				.unwrap_or(true)
		})
		.unwrap_or(true)
}

/// The argument prefix every `rg` spawn in this kern shares, with the
/// caller's own globs in the one position where they are safe.
///
/// Dot-directories are searched: this workspace keeps its memo record, its
/// hooks and its tests under them, and a search that cannot see
/// them answers a false empty, which reads exactly like a true one. `.git/` is
/// the single exclusion, because the composition nests submodules whose object
/// stores would otherwise outnumber every real result; `.gitignore` still
/// removes `target/` and `node_modules/`.
fn base_argv(globs: &[&str]) -> Vec<String> {
	let mut argv = vec![
		"--no-config".to_string(),
		"--no-follow".to_string(),
		"--hidden".to_string(),
	];
	for glob in globs {
		argv.push("--glob".to_string());
		argv.push((*glob).to_string());
	}
	// Last, because rg lets the last matching glob decide. A caller asking for
	// `*` matches the object store's file names too, and would re-include
	// everything this excludes if the exclusion came first.
	argv.push("--glob=!.git/".to_string());
	argv
}

/// Enumerate workspace files with `rg --files --glob PATTERN`. Ignored files
/// stay excluded and symlinks are not followed; dot-directories other than
/// `.git/` are walked, per `base_argv`.
async fn enumerate(
	service: &Arc<Service>,
	context: &Context,
	cwd: &Path,
	cancel: &std::sync::atomic::AtomicBool,
	pattern: Option<&str>,
) -> Result<Vec<String>, String> {
	let pattern = pattern.unwrap_or("*");
	let mut argv = base_argv(&[pattern]);
	argv.push("--files".to_string());
	argv.push("--null".to_string());
	let raw = run_rg(service, context, cwd, cancel, argv).await?;
	if !raw.is_empty() && !raw.ends_with('\0') {
		return Err("malformed rg output: expected NUL-delimited paths".into());
	}
	let mut lines: Vec<String> = raw.split_terminator('\0').map(str::to_owned).collect();
	for change in session_changes(service, context, cwd)? {
		lines.retain(|line| line != &change.path);
		if change.content.is_some() && glob_matches(&change.path, Some(pattern)) {
			lines.push(change.path);
		}
	}
	lines.sort();
	lines.dedup();
	Ok(lines)
}

/// Grep with line numbers over explicit absolute paths (already resolved and
/// validated) using rg JSON events; malformed output fails explicitly.
pub async fn grep_paths(
	service: &Arc<Service>,
	context: &Context,
	cwd: &Path,
	cancel: &std::sync::atomic::AtomicBool,
	pattern: &str,
	paths: &[String],
	glob: Option<&str>,
) -> Result<Vec<MatchItem>, String> {
	let mut argv = base_argv(&glob.into_iter().collect::<Vec<_>>());
	argv.push("--json".to_string());
	argv.push("--".to_string());
	argv.push(pattern.to_string());
	let changes = session_changes(service, context, cwd)?;
	// A path only the session branch has is searched below, not by rg.
	let on_disk: Vec<&String> = paths
		.iter()
		.filter(|p| changes.is_empty() || cwd.join(p).exists())
		.collect();
	let raw = if paths.is_empty() {
		argv.push(".".into());
		run_rg(service, context, cwd, cancel, argv).await?
	} else if on_disk.is_empty() {
		String::new()
	} else {
		argv.extend(on_disk.into_iter().cloned());
		run_rg(service, context, cwd, cancel, argv).await?
	};
	let mut items: Vec<MatchItem> = Vec::new();
	for line in raw.lines() {
		// Every event must be valid JSON; only `match` events carry the
		// fields we consume, and those must be well-formed or the whole
		// result fails rather than being presented partial.
		let event: Value =
			serde_json::from_str(line).map_err(|e| format!("malformed rg output: {e}"))?;
		if event.get("type") != Some(&json!("match")) {
			continue;
		}
		let path = event["data"]["path"]["text"]
			.as_str()
			.ok_or_else(|| "malformed rg output: missing path".to_string())?;
		let line_number = event["data"]["line_number"]
			.as_u64()
			.ok_or_else(|| "malformed rg output: missing line number".to_string())?;
		let text = event["data"]["lines"]["text"]
			.as_str()
			.ok_or_else(|| "malformed rg output: missing lines".to_string())?
			.trim_end_matches('\n')
			.to_string();
		items.push(MatchItem {
			path: Path::new(path)
				.strip_prefix(cwd)
				.unwrap_or(Path::new(path))
				.to_string_lossy()
				.trim_start_matches("./")
				.to_string(),
			line: line_number,
			text,
		});
	}
	if !changes.is_empty() {
		let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
		for change in changes {
			items.retain(|item| item.path != change.path);
			let in_scope = paths.is_empty()
				|| paths
					.iter()
					.any(|p| cwd.join(&change.path).starts_with(cwd.join(p)));
			let Some(text) = change
				.content
				.and_then(|bytes| String::from_utf8(bytes).ok())
			else {
				continue;
			};
			if !in_scope || !glob_matches(&change.path, glob) {
				continue;
			}
			for (index, line) in text.lines().enumerate() {
				if re.is_match(line) {
					items.push(MatchItem {
						path: change.path.clone(),
						line: index as u64 + 1,
						text: line.to_string(),
					});
				}
			}
		}
	}
	items.sort_by(|a, b| (&a.path, a.line, &a.text).cmp(&(&b.path, b.line, &b.text)));
	items.dedup();
	Ok(items)
}

/// One path the session changed, workspace-relative; no content when deleted.
struct Change {
	path: String,
	content: Option<Vec<u8>>,
}

/// In git mode, the session's changed paths under the workspace, so searches
/// see its branch before the worktree. Dot-directories are included, as they
/// are for the walker: an overlay is hidden from a search only when the walker
/// hides it too, or the two halves of one answer disagree.
fn session_changes(
	service: &Service,
	context: &Context,
	cwd: &Path,
) -> Result<Vec<Change>, String> {
	if service.config.mode != Mode::Git {
		return Ok(Vec::new());
	}
	let Ok(store) = service.store.at_readonly(cwd) else {
		return Ok(Vec::new());
	};
	let root = store.root().canonicalize().map_err(|e| e.to_string())?;
	let mut changes = Vec::new();
	for (status, rel) in store.owned_paths(&context.session)? {
		let Ok(path) = root.join(&rel).strip_prefix(cwd).map(Path::to_path_buf) else {
			continue;
		};
		let content = match status {
			'D' => None,
			_ => store.read(&context.session, Path::new(&rel))?,
		};
		changes.push(Change {
			path: path.to_string_lossy().into_owned(),
			content,
		});
	}
	Ok(changes)
}

/// rg's `--glob` reading: a pattern without `/` matches the file name at any
/// depth, one with `/` matches the whole workspace-relative path.
fn glob_matches(path: &str, pattern: Option<&str>) -> bool {
	match pattern {
		Some(p) if !p.contains('/') => {
			matches_glob(path.rsplit('/').next().unwrap_or(path), Some(p))
		}
		other => matches_glob(path, other),
	}
}

/// Spawn `rg` with an argument array, drain stdout concurrently, and enforce
/// the deadline and cancellation. Exit 1 is an empty success; other nonzero
/// exits carry diagnostics. The child is killed and reaped on deadline or
/// cancel; a missing executable is an actionable error.
async fn run_rg(
	service: &Arc<Service>,
	_context: &Context,
	cwd: &Path,
	cancel: &std::sync::atomic::AtomicBool,
	argv: Vec<String>,
) -> Result<String, String> {
	check_cancel(cancel)?;
	let mut command = Command::new(&service.config.rg_program);
	command
		.args(&argv)
		.current_dir(cwd)
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.kill_on_drop(true);
	command.env_remove("RIPGREP_CONFIG_PATH");
	let mut child = command
		.spawn()
		.map_err(|e| format!("search backend unavailable (is rg installed?): {e}"))?;
	let stdout = child.stdout.take().expect("stdout piped");
	let stderr = child.stderr.take().expect("stderr piped");
	let deadline = tokio::time::sleep(Duration::from_millis(service.config.max_search_wait_ms));
	tokio::pin!(deadline);
	let result = {
		let collect = async {
			let (raw, diagnostics, status) = tokio::try_join!(
				read_bounded(stdout, service.config.max_search_backend_bytes),
				read_bounded(stderr, 64 * 1024),
				async {
					child
						.wait()
						.await
						.map_err(|e| format!("search backend failed: {e}"))
				}
			)?;
			match status.code() {
				Some(0) | Some(1) => {}
				Some(code) => return Err(backend_failure(code, &diagnostics)),
				None => return Err("search backend was killed by a signal".into()),
			}
			String::from_utf8(raw).map_err(|e| format!("malformed rg output: {e}"))
		};
		tokio::pin!(collect);
		loop {
			tokio::select! {
				_ = &mut deadline => break Err(format!("search exceeded {} ms deadline; child terminated", service.config.max_search_wait_ms)),
				_ = tokio::time::sleep(Duration::from_millis(5)) => {
					if check_cancel(cancel).is_err() { break Err("cancelled".into()); }
				},
				result = &mut collect => break result,
			}
		}
	};
	if result.is_err() {
		let _ = child.start_kill();
		let _ = child.wait().await;
	}
	check_cancel(cancel)?;
	result
}

/// A nonzero rg exit names rg's own stderr on one line (a regex rg refuses,
/// e.g. a literal `\n` without multiline mode, says why there).
fn backend_failure(code: i32, stderr: &[u8]) -> String {
	let said = String::from_utf8_lossy(stderr);
	let said: Vec<&str> = said
		.lines()
		.map(str::trim)
		.filter(|l| !l.is_empty())
		.collect();
	let said: String = said.join(" ").chars().take(600).collect();
	if said.is_empty() {
		format!("search backend failed with exit code {code}")
	} else {
		format!("search backend failed with exit code {code}: {said}")
	}
}

async fn read_bounded(
	reader: impl tokio::io::AsyncRead + Unpin,
	limit: usize,
) -> Result<Vec<u8>, String> {
	let mut bytes = Vec::new();
	reader
		.take(limit as u64 + 1)
		.read_to_end(&mut bytes)
		.await
		.map_err(|e| format!("search backend read failed: {e}"))?;
	if bytes.len() > limit {
		return Err(format!(
			"capacity_exceeded: search backend output exceeds {limit} bytes; narrow the query"
		));
	}
	Ok(bytes)
}

#[cfg(test)]
#[path = "../tests/unit/search/tests.rs"]
mod tests;
