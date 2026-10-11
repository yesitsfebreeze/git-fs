use crate::change_record::Evidence;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Every field is required, because by the time this is read the host has
/// checked each one against the declaration in [`MANIFEST`]. There is no
/// `Default` and no per-field fallback: a second copy of the numbers in Rust is
/// how the two drift apart.
/// Where writes and edits publish; see the `mode` setting.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
	Git,
	Local,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
	pub mode: Mode,
	pub store_dir: String,
	pub integrate: Value,
	pub limits: crate::limits::Limits,
	pub max_read_bytes: usize,
	pub max_read_lines: usize,
	pub max_whole_read_lines: usize,
	pub max_file_bytes: usize,
	pub max_invocations: usize,
	pub max_runs: usize,
	pub max_reads: usize,
	pub max_search_items: usize,
	pub max_search_bytes: usize,
	pub max_search_wait_ms: u64,
	pub max_search_backend_bytes: usize,
	pub max_search_results_bytes: usize,
	pub max_chains: usize,
	pub rg_program: String,
}

/// This kern's own document, embedded. The declarations in it are the only
/// place its defaults and bounds are written; [`Config::parse`] reads them back
/// for a run with no host to settle the configuration.
/// The manifest, as the loader reads it: `kern.md` is the document now, and
/// its frontmatter is the JSON this settles against. Written as one JSON
/// object on purpose — JSON is YAML, so the loader reads the same bytes.
pub const MANIFEST: &str = include_str!("../manifest.md");

/// The frontmatter of [`MANIFEST`], between the first pair of `---` lines.
pub fn document() -> &'static str {
	static DOC: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
	DOC.get_or_init(|| {
		let rest = MANIFEST
			.strip_prefix("---\n")
			.expect("the manifest opens with frontmatter");
		let end = rest
			.find("\n---\n")
			.expect("the manifest closes its frontmatter");
		&rest[..end]
	})
}

/// The declared configuration, for tests that build one without a host.
///
/// This is not a second copy of the numbers the type refuses: it reads the same
/// declaration [`Config::parse`] does. Nothing shipped calls it — `parse` is the
/// way in — but it is not gated, because a kern's declaration is worth the
/// same answer to a test in another crate as to one of its own.
impl Default for Config {
	fn default() -> Self {
		Self::parse(serde_json::json!({})).expect("this kern's own declaration parses")
	}
}

impl Config {
	pub fn parse(value: Value) -> Result<Self, String> {
		// What the host configured, over what this kern declares. The host
		// normally sets every key already, so the declared table underneath
		// changes nothing; it is what makes a run with no host — a unit test, a
		// standalone invocation — read the values this kern actually ships.
		let mut merged = crate::config::declared(document());
		crate::config::merge(&mut merged, value);
		let config: Self = serde_json::from_value(merged).map_err(|e| e.to_string())?;
		// How large each cap may be is the declaration's business, not this
		// parser's: the bounds live once, in `kern.md`, and the host has
		// already refused anything outside them. What is checked here is only
		// what a cap being *zero* would break — a budget of nothing is not a
		// small budget, it is a filesystem that answers nothing.
		let caps = [
			config.max_read_bytes,
			config.max_read_lines,
			config.max_whole_read_lines,
			config.max_file_bytes,
			config.max_invocations,
			config.max_runs,
			config.max_reads,
			config.max_search_items,
			config.max_search_bytes,
			config.max_search_backend_bytes,
			config.max_search_results_bytes,
			config.max_chains,
		];
		if caps.contains(&0)
			|| config.max_search_wait_ms == 0
			|| config.rg_program.is_empty()
			|| config.store_dir.is_empty()
		{
			return Err("invalid filesystem caps".into());
		}
		Ok(config)
	}
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Context {
	pub session: String,
	pub run: String,
	pub call: String,
	pub cwd: String,
}

impl Context {
	pub fn validate(&self) -> Result<(), String> {
		for value in [&self.session, &self.run, &self.call] {
			if value.trim().is_empty() || value.len() > 256 || value.contains('\0') {
				return Err("invalid invocation context".into());
			}
		}
		if !Path::new(&self.cwd).is_absolute() || self.cwd.contains('\0') || self.cwd.len() > 4096 {
			return Err("context.cwd must be an absolute workspace directory".into());
		}
		Ok(())
	}
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum Request {
	Describe,
	Call { context: Context, input: Value },
	Cancel { context: Context },
}

#[derive(Clone, PartialEq, Eq)]
pub enum Read {
	Absent,
	Present([u8; 32]),
}

#[derive(Default)]
struct Run {
	retired: bool,
	calls: HashMap<(String, String), Arc<AtomicBool>>,
	reads: HashMap<PathBuf, Read>,
	chains: Vec<(String, crate::search::ResultSet, usize)>,
}

#[derive(Default)]
struct State {
	runs: HashMap<(String, String), Run>,
	active: HashMap<String, String>,
	targets: HashMap<PathBuf, Arc<Mutex<()>>>,
}

pub type Touch =
	Arc<dyn Fn(Context, Evidence) -> BoxFuture<'static, Result<(), String>> + Send + Sync>;

#[cfg(test)]
type BeforeCommit = Arc<dyn Fn(&Path) + Send + Sync>;

pub struct Service {
	pub search_epoch: String,
	pub store: Arc<crate::store::StoreConfig>,
	#[cfg(test)]
	before_commit: Option<BeforeCommit>,
	#[cfg(test)]
	after_publish: Option<BeforeCommit>,
	pub config: Config,
	state: Mutex<State>,
	touch: Touch,
}

impl Service {
	pub fn new(config: Config, touch: Touch) -> Self {
		static EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
		Self {
			search_epoch: format!(
				"{}:{:?}:{}",
				std::process::id(),
				std::time::SystemTime::now(),
				EPOCH.fetch_add(1, Ordering::Relaxed)
			),
			store: Arc::new(crate::store::StoreConfig::new(&config.store_dir)),
			#[cfg(test)]
			before_commit: None,
			#[cfg(test)]
			after_publish: None,
			config,
			state: Mutex::default(),
			touch,
		}
	}

	pub(crate) fn before_commit(&self, path: &Path) {
		#[cfg(test)]
		if let Some(hook) = &self.before_commit {
			hook(path);
		}
		#[cfg(not(test))]
		let _ = path;
	}

	pub(crate) fn after_publish(&self, path: &Path) {
		#[cfg(test)]
		if let Some(hook) = &self.after_publish {
			hook(path);
		}
		#[cfg(not(test))]
		let _ = path;
	}

	#[allow(dead_code)]
	pub fn cancelled(&self, context: &Context) -> bool {
		let state = self.state.lock().unwrap();
		state
			.runs
			.get(&(context.session.clone(), context.run.clone()))
			.and_then(|run| run.calls.get(&(context.call.clone(), context.cwd.clone())))
			.is_some_and(|cancel| cancel.load(Ordering::SeqCst))
	}

	pub async fn dispatch(self: &Arc<Self>, tool: &'static str, request: Value) -> Value {
		let request = match serde_json::from_value::<Request>(request) {
			Ok(request) => request,
			Err(e) => return failure(e.to_string()),
		};
		match request {
			Request::Describe => describe(tool),
			Request::Cancel { context } => {
				match context.validate().and_then(|()| self.cancel(&context)) {
					Ok(()) => success(json!(
						"cancelled invocation; completed changes are not rolled back"
					)),
					Err(e) => failure(e),
				}
			}
			Request::Call {
				mut context,
				mut input,
			} => {
				if let Err(e) = context.validate() {
					return failure(e);
				}
				// A read may name a peer: `session` moves the view to that
				// session's branch. A mutation never takes it.
				if matches!(tool, "read" | "glob" | "grep" | "search") {
					if let Some(peer) = input.as_object_mut().and_then(|f| f.remove("session")) {
						context.session = peer.as_str().unwrap_or_default().to_owned();
						if let Err(e) = context.validate() {
							return failure(e);
						}
					}
				}
				// Search tools are async compositional chains; mutations use
				// the blocking files worker. Both register the invocation for
				// cancellation before any work starts.
				match tool {
					"search" | "glob" | "grep" => {
						let cancel = match self.register(&context) {
							Ok(cancel) => cancel,
							Err(e) => return failure(e),
						};
						let chain = match crate::search::as_chain(tool, input) {
							Ok(chain) => chain,
							Err(e) => return failure(e),
						};
						return match crate::search::run_chain(self, &context, &cancel, chain).await
						{
							Ok(outcome) => {
								let complete = !outcome.truncated;
								let kind = outcome.result.kind();
								success(json!({
									"kind": kind,
									"items": outcome.result.to_value(),
									"ref": outcome.stored,
									"count": outcome.result.len(),
									"complete": complete,
									"truncated": outcome.truncated,
									"next_cursor": outcome.next_cursor,
									"results_revision": outcome.revision,
									"total_count": outcome.total,
									"consistency": "captured_results",
									"tree_changes": "not_revalidated; start a new search for current results",
								}))
							}
							Err(e) => failure(e),
						};
					}
					_ => {}
				}
				// Reject model fields and malformed context before touching paths.
				let input = match crate::files::Input::parse(tool, input) {
					Ok(input) => input,
					Err(e) => return failure(e),
				};
				let cancel = match self.register(&context) {
					Ok(cancel) => cancel,
					Err(e) => return failure(e),
				};
				let service = self.clone();
				let worker_context = context.clone();
				let outcome = tokio::task::spawn_blocking(move || {
					crate::files::execute(&service, &worker_context, input, &cancel)
				})
				.await;
				match outcome {
					Ok(Ok(outcome)) => {
						if let Some(evidence) = outcome.evidence {
							let path = evidence.target.path.clone();
							let recorded = tokio::time::timeout(
								std::time::Duration::from_secs(2),
								(self.touch)(context, evidence),
							)
							.await;
							let error=match recorded {Ok(Ok(()))=>None,Ok(Err(e))=>Some(format!("sessions attribution failed or unconfirmed: {e}")),Err(_)=>Some("sessions attribution deadline exceeded; recording outcome unknown".into())};
							if let Some(error) = error {
								return failure(format!("partial_success: file already changed: {path}; {error}. {} Do not automatically retry.",outcome.partial.unwrap_or_default()));
							}
						}
						if let Some(error) = outcome.partial {
							return failure(error);
						}

						let mut answer = success(json!(outcome.content));
						if let Some(revision) = outcome.revision {
							answer["revision"] = revision;
						}
						answer
					}
					Ok(Err(error)) => failure(error),
					Err(error) => failure(format!("filesystem worker failed: {error}")),
				}
			}
		}
	}

	fn run<'a>(&self, state: &'a mut State, context: &Context) -> Result<&'a mut Run, String> {
		let key = (context.session.clone(), context.run.clone());
		if !state.runs.contains_key(&key) && state.runs.len() >= self.config.max_runs {
			return Err("capacity_exceeded: run registry".into());
		}
		Ok(state.runs.entry(key).or_default())
	}

	fn cancel(&self, context: &Context) -> Result<(), String> {
		let mut state = self.state.lock().unwrap();
		let run = self.run(&mut state, context)?;
		let key = (context.call.clone(), context.cwd.clone());
		if !run.calls.contains_key(&key) && run.calls.len() >= self.config.max_invocations {
			return Err("capacity_exceeded: cancellation tombstones".into());
		}
		run.calls
			.entry(key)
			.or_default()
			.store(true, Ordering::SeqCst);
		Ok(())
	}

	fn register(&self, context: &Context) -> Result<Arc<AtomicBool>, String> {
		let mut state = self.state.lock().unwrap();
		let run = self.run(&mut state, context)?;
		if run.retired {
			return Err("run_disposed: start a new unique run".into());
		}
		let key = (context.call.clone(), context.cwd.clone());
		if let Some(cancel) = run.calls.get(&key) {
			return Err(if cancel.load(Ordering::SeqCst) {
				"cancelled"
			} else {
				"duplicate_invocation"
			}
			.into());
		}
		if run.calls.len() >= self.config.max_invocations {
			return Err("capacity_exceeded: invocation registry".into());
		}
		let cancel = Arc::new(AtomicBool::new(false));
		run.calls.insert(key, cancel.clone());
		if let Some(old) = state
			.active
			.insert(context.session.clone(), context.run.clone())
		{
			if old != context.run {
				if let Some(old) = state.runs.get_mut(&(context.session.clone(), old)) {
					old.retired = true;
					old.reads.clear();
					old.chains.clear();
					for cancel in old.calls.values() {
						cancel.store(true, Ordering::SeqCst);
					}
				}
			}
		}
		Ok(cancel)
	}

	pub fn target(&self, path: &Path) -> Arc<Mutex<()>> {
		self.state
			.lock()
			.unwrap()
			.targets
			.entry(path.to_path_buf())
			.or_default()
			.clone()
	}

	pub fn mark_read(&self, context: &Context, path: &Path, read: Read) -> Result<(), String> {
		let mut state = self.state.lock().unwrap();
		let run = self.run(&mut state, context)?;
		if run.retired {
			return Err("cancelled: run disposed".into());
		}
		if !run.reads.contains_key(path) && run.reads.len() >= self.config.max_reads {
			return Err("capacity_exceeded: file reads".into());
		}
		run.reads.insert(path.to_path_buf(), read);
		Ok(())
	}

	pub fn last_read(&self, context: &Context, path: &Path) -> Result<Option<Read>, String> {
		let mut state = self.state.lock().unwrap();
		let run = self.run(&mut state, context)?;
		if run.retired {
			return Err("cancelled: run disposed".into());
		}
		if !run.reads.contains_key(path) && run.reads.len() >= self.config.max_reads {
			return Err("capacity_exceeded: file reads".into());
		}
		Ok(run.reads.get(path).cloned())
	}

	/// Store a chain result under the run's chain store, scoped to
	/// (session, run, cwd). Returns the index a later `ref` resolves to.
	pub fn store_chain_result(
		&self,
		context: &Context,
		result: crate::search::ResultSet,
	) -> Result<usize, String> {
		let bytes = result.to_value().to_string().len();
		let mut state = self.state.lock().unwrap();
		let retained: usize = state
			.runs
			.values()
			.flat_map(|r| &r.chains)
			.map(|(_, _, bytes)| bytes)
			.sum();
		if bytes
			> self
				.config
				.max_search_results_bytes
				.saturating_sub(retained)
		{
			return Err(
				"capacity_exceeded: search results; narrow the query or start a new run".into(),
			);
		}
		let run = self.run(&mut state, context)?;
		if run.retired {
			return Err("cancelled: run disposed".into());
		}
		if run.chains.len() >= self.config.max_chains {
			return Err("capacity_exceeded: chain results".into());
		}
		run.chains.push((context.cwd.clone(), result, bytes));
		Ok(run.chains.len() - 1)
	}

	/// Resolve a `ref` against the run's chain store. A foreign `cwd` fails:
	/// refs are scoped to the trusted session/run/cwd they were made under.
	pub fn chain_result(
		&self,
		context: &Context,
		index: usize,
	) -> Result<Option<crate::search::ResultSet>, String> {
		let mut state = self.state.lock().unwrap();
		let run = self.run(&mut state, context)?;
		if run.retired {
			return Err("cancelled: run disposed".into());
		}
		match run.chains.get(index) {
			Some((cwd, result, _)) if cwd == &context.cwd => Ok(Some(result.clone())),
			Some(_) => Err("ref is scoped to another cwd".into()),
			None => Ok(None),
		}
	}
}

pub fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
	if cancel.load(Ordering::SeqCst) {
		Err("cancelled".into())
	} else {
		Ok(())
	}
}

/// The tool event answers `{content, error}` with the answer itself in `content`
/// — an object for structured answers, a plain string for text and messages,
/// never stringified JSON a consumer must parse twice.
fn success(content: Value) -> Value {
	json!({"content":content,"error":false})
}
fn failure(content: impl Into<String>) -> Value {
	json!({"content":content.into(),"error":true})
}

pub(crate) fn describe(tool: &str) -> Value {
	let (description, required, properties, any_of) = match tool {
		"read" => ("Read numbered UTF-8 lines; in git mode read session changes, then committed trunk, then untracked disk files. The response revision identifies the selected view and disk divergence. A partial view establishes freshness, not whole-file review. A file over max_whole_read_lines is refused unless offset or limit is given: read a range, grep, .", json!(["path"]), json!({"path":{"type":"string"},"offset":{"type":"integer","minimum":1},"limit":{"type":"integer","minimum":1},"session":{"type":"string","description":"A peer session whose branch to view instead of your own."},"raw":{"type":"boolean","description":"Answer the file's exact text without line numbers or the whole-read line bound; the byte bound still applies."}}), Value::Null),
		"write" => ("Create a file or overwrite the current version it read; In git mode it is one commit on your session branch and the worktree is untouched until gitfs_materialize, so nothing runs until then; gitfs_rollback undoes it.", json!(["path","text"]), json!({"path":{"type":"string"},"text":{"type":"string"}}), Value::Null),
		"edit" => ("Replace an exact unique match in a current file it read, every match with all=true, or apply an edits array of old/new (or oldText/newText) pairs in one all-or-nothing write. In git mode it is one commit on your session branch.", json!(["path"]), json!({"path":{"type":"string"},"old":{"type":"string","minLength":1},"new":{"type":"string"},"all":{"type":"boolean"},"edits":{"type":"array","minItems":1,"items":{"type":"object","properties":{"old":{"type":"string","minLength":1},"new":{"type":"string"},"oldText":{"type":"string","minLength":1},"newText":{"type":"string"},"all":{"type":"boolean"}},"additionalProperties":false,"anyOf":[{"required":["old","new"]},{"required":["oldText","newText"]}]}}}), json!([{"required":["old","new"]},{"required":["edits"]}])),
		"search" => ("Search typed stages or resume next_cursor with cursor alone. Pages capture results once; tree changes require a new search. ref reuses the full captured result of this run.", json!([]), json!({"cursor":{"type":"string"},"ref":{"type":"integer","minimum":0},"stages":{"type":"array","items":{"type":"object"},"minItems":0},"session":{"type":"string","description":"A peer session whose branch to view instead of your own."}}), Value::Null),
		"glob" => ("List workspace files matching an rg glob, sorted, workspace-relative; in git mode your session's added and deleted files count.", json!([]), json!({"pattern":{"type":"string"},"session":{"type":"string","description":"A peer session whose branch to view instead of your own."}}), Value::Null),
		"grep" => ("Search file contents with rg regex syntax; returns path, line number and text per match; in git mode your session's changed files are searched as committed.", json!(["pattern"]), json!({"pattern":{"type":"string"},"path":{"type":"string"},"glob":{"type":"string"},"session":{"type":"string","description":"A peer session whose branch to view instead of your own."}}), Value::Null),
		_ => return failure("unknown tool"),
	};
	// `reads` is this tool's own word on whether a call changes anything; the
	// agent parallelises on it and carries no list of its own.
	let reads = matches!(tool, "read" | "search" | "glob" | "grep");
	let mut input_schema = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
	if !any_of.is_null() {
		input_schema["anyOf"] = any_of;
	}
	json!({"name":tool,"description":description,"reads":reads,"input_schema":input_schema})
}

#[cfg(test)]
#[path = "../tests/unit/service/tests.rs"]
mod tests;
