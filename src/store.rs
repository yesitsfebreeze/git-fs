//! Session overlay store: one branch `refs/gitfs/<session>` per agent session,
//! written through git plumbing so the working tree and the real index are
//! never touched. Integrate overlays the session's owned paths onto the current
//! branch head, so trunk movement between session start and integrate can't show
//! stale full trees.

#[path = "snapshot.rs"]
mod snapshot;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::change_record::{Operation, Storage, Version};
use crate::provenance::{self, Capture};
use crate::Result;

/// An owned path and its diff status (`A`/`M`/`D`).
pub type Owned = (char, String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewedTree {
	pub repository: String,
	pub branch: String,
	pub head: String,
	pub index_revision: String,
	pub overlay: String,
	pub tree: String,
	pub owned: Vec<Owned>,
}

/// Durable, operation-bound recovery state. Written before any ref transaction.
/// An absent commit is intentionally ambiguous, never permission to dispatch again.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrationAttempt {
	pub schema: String,
	pub revision: String,
	pub session: String,
	pub review: ReviewedTree,
	pub subject: String,
	pub author_name: String,
	pub author_email: String,
	pub committed: Option<String>,
	pub worktree: Option<Value>,
}

fn publication_failure(
	dispatched: bool,
	commit: &str,
	actual_head: Option<&str>,
	error: &str,
) -> String {
	if actual_head == Some(commit) {
		format!("partial_success: published commit {commit}; ref transaction response failed. Reconcile before retry: {error}")
	} else if dispatched {
		format!("publication outcome unknown for attempted commit {commit}; reconcile before retry: {error}")
	} else {
		format!("publication refused before dispatch; preview again: {error}")
	}
}

/// How long a shared-index update waits for `.git/index.lock` held by someone
/// else. Git's own readers (`git status` from an editor, a prompt or the buffer
/// git provider) take the lock for a few milliseconds to refresh stat data, so
/// a single attempt refuses whenever one of them is mid-refresh. Git-fs never
/// holds this lock itself: the integrate commit is built from a tree, not the
/// index, so the index is only touched by the post-integrate sync, briefly.
const INDEX_PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);
/// Attempts at a shared-index update: a lock held longer is a real writer, and
/// the path stays in `kept` instead of blocking every other agent.
const INDEX_ATTEMPTS: usize = 20;

/// File locks coordinate threads and kern processes. Session indexes are
/// independent; only working-tree and trunk changes share the repository lock.
struct MutationLock(std::fs::File);
impl Drop for MutationLock {
	fn drop(&mut self) {
		let _ = self.0.unlock();
	}
}

/// How many session branches a `known_revision` check reads before giving up.
const MAX_KNOWN_REVISIONS: usize = 64;

const IDENT_NAME: &str = "kern gitfs";
const IDENT_EMAIL: &str = "gitfs@kern.local";
const LIVE_REF: &str = "refs/gitfs-live/current";

#[derive(Clone)]
pub struct Store {
	root: PathBuf,
	dir: PathBuf,
	/// Set when the project has no repository of its own: the store directory
	/// is then the git directory, holding only session branches and their
	/// objects, with `root` as its worktree.
	git_dir: Option<PathBuf>,
}

pub struct StoreConfig {
	store_dir: String,
}

impl StoreConfig {
	pub fn directory(&self) -> &str {
		&self.store_dir
	}

	pub fn new(store_dir: &str) -> Self {
		Self {
			store_dir: store_dir.to_owned(),
		}
	}

	/// Resolve the repo root from a caller cwd (which may be a checkout or any
	/// directory inside it) and the runtime store dir under it. A directory
	/// outside every repository gets a store that is its own git directory.
	pub fn at(&self, cwd: &Path) -> Result<Store> {
		let store = match self.at_readonly(cwd) {
			Ok(store) => store,
			Err(_) => self.init_standalone(cwd)?,
		};
		self.prepare(store)
	}

	/// As `at`, but never makes a standalone store: for callers that observe
	/// (the hooks), so that only a git-fs write creates one.
	pub fn at_existing(&self, cwd: &Path) -> Result<Store> {
		self.prepare(self.at_readonly(cwd)?)
	}

	fn prepare(&self, store: Store) -> Result<Store> {
		std::fs::create_dir_all(&store.dir).map_err(|e| format!("cannot create store dir: {e}"))?;
		// A store that is its own git directory has no `.git/info/exclude`.
		if store.git_dir.is_none() {
			exclude_private(&store.root, &self.store_dir);
		}
		Ok(store)
	}

	pub fn at_readonly(&self, cwd: &Path) -> Result<Store> {
		if let Ok(s) = run(cwd, None, &["rev-parse", "--show-toplevel"], None, None) {
			// Git prints `/` separators on Windows; rebuild in native form.
			let root: PathBuf = Path::new(String::from_utf8_lossy(&s).trim())
				.components()
				.collect();
			if !root.as_os_str().is_empty() {
				let dir = root.join(&self.store_dir);
				return Ok(Store {
					root,
					dir,
					git_dir: None,
				});
			}
		}
		// No project repository: the nearest store that is a git directory.
		cwd.ancestors()
			.map(|root| (root, root.join(&self.store_dir)))
			.find(|(_, dir)| {
				dir.join("HEAD").is_file()
					&& owner_is_user(dir)
					&& owner_is_user(&dir.join("config"))
			})
			.map(|(root, dir)| Store {
				root: root.to_path_buf(),
				git_dir: Some(dir.clone()),
				dir,
			})
			.ok_or_else(|| {
				"no git repository and no git-fs store here yet; the first write creates one"
					.to_string()
			})
	}

	/// Make the store directory under `cwd` a git directory of its own.
	fn init_standalone(&self, cwd: &Path) -> Result<Store> {
		let dir = cwd.join(&self.store_dir);
		if dir.exists() && !owner_is_user(&dir) {
			return Err(format!(
				"{} belongs to another user; refusing to use it as a git directory",
				dir.display()
			));
		}
		// No templates: the store holds branches and objects only, and two first
		// calls at once would otherwise fail copying the same template files.
		run(cwd, Some(&dir), &["init", "-q", "--template="], None, None)?;
		// rg honours `.gitignore` only inside a repository, `.ignore` always:
		// keep the object store out of every search.
		std::fs::write(dir.join(".ignore"), "*\n")
			.map_err(|e| format!("cannot create store dir: {e}"))?;
		Ok(Store {
			root: cwd.to_path_buf(),
			git_dir: Some(dir.clone()),
			dir,
		})
	}
}

/// Whether the current user owns `path`. A standalone store is handed to git
/// as an explicit `GIT_DIR`, which skips git's own dubious-ownership check,
/// and its config can name commands git runs (`core.fsmonitor`): one that
/// arrived with someone else's directory tree must not be adopted.
#[cfg(unix)]
fn owner_is_user(path: &Path) -> bool {
	use std::os::unix::fs::MetadataExt;
	// SAFETY: geteuid takes no arguments and cannot fail.
	let uid = unsafe { libc::geteuid() };
	std::fs::symlink_metadata(path).is_ok_and(|m| m.uid() == uid)
}

/// Ceiling: no ownership check on Windows; add one through the file's
/// security descriptor if standalone stores are used there.
#[cfg(not(unix))]
fn owner_is_user(_path: &Path) -> bool {
	true
}

#[derive(Debug)]
pub struct Materialize {
	pub applied: Vec<String>,
	/// Paths whose disk copy and session copy were merged three-way before
	/// landing; they are also in `applied`.
	pub merged: Vec<String>,
	pub conflicts: Vec<(String, String)>,
	pub errors: Vec<String>,
}

impl Materialize {
	pub fn to_value(&self) -> Value {
		json!({
			"applied": self.applied,
			"merged": self.merged,
			"conflicts": self.conflicts.iter().map(|(p, r)| json!({"path": p, "reason": r})).collect::<Vec<_>>(),
			"errors": self.errors,
		})
	}
}

fn trimmed_opt(s: String) -> Option<String> {
	let t = s.trim();
	if t.is_empty() {
		None
	} else {
		Some(t.to_owned())
	}
}

/// Git takes repository-relative paths with `/` separators on every platform:
/// `git update-index --cacheinfo` rejects a `\` path as an invalid path on Windows.
fn printable(p: &Path) -> String {
	p.to_string_lossy().replace('\\', "/")
}

/// Base git command: fixed identity, hooks disabled, optional private index.
/// No porcelain checkout runs here: blobs are staged by sha and written to disk
/// as their exact bytes, so `core.autocrlf` and attribute filters never rewrite
/// content behind the branch's back.
/// `git_dir` names the store's own git directory when the project has none.
fn git_cmd(
	root: &Path,
	git_dir: Option<&Path>,
	args: &[&str],
	index: Option<&Path>,
) -> Result<Command> {
	#[cfg(windows)]
	let normalized_root = PathBuf::from(crate::hook::git_command_path(root)?);
	#[cfg(windows)]
	let root = normalized_root.as_path();
	let mut cmd = Command::new("git");
	cmd.arg("-C")
		.arg(root)
		.arg("-c")
		.arg("core.hooksPath=/dev/null")
		.args(args)
		.env("GIT_AUTHOR_NAME", IDENT_NAME)
		.env("GIT_AUTHOR_EMAIL", IDENT_EMAIL)
		.env("GIT_COMMITTER_NAME", IDENT_NAME)
		.env("GIT_COMMITTER_EMAIL", IDENT_EMAIL);
	if let Some(index) = index {
		#[cfg(windows)]
		let index = crate::hook::git_command_path(index)?;
		cmd.env("GIT_INDEX_FILE", index);
	}
	if let Some(dir) = git_dir {
		#[cfg(windows)]
		let dir = crate::hook::git_command_path(dir)?;
		cmd.env("GIT_DIR", dir).env("GIT_WORK_TREE", root);
	}
	Ok(cmd)
}

/// Run git against `root` with a fixed identity, hooks disabled, and an
/// optional private index file (never the working tree's index).
fn run(
	root: &Path,
	git_dir: Option<&Path>,
	args: &[&str],
	index: Option<&Path>,
	stdin: Option<&[u8]>,
) -> Result<Vec<u8>> {
	let mut cmd = git_cmd(root, git_dir, args, index)?;
	if stdin.is_some() {
		cmd.stdin(Stdio::piped());
	}
	cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
	let mut child = cmd.spawn().map_err(|e| format!("git spawn failed: {e}"))?;
	if let Some(bytes) = stdin {
		use std::io::Write;
		if let Some(mut si) = child.stdin.take() {
			si.write_all(bytes).map_err(|e| format!("git stdin: {e}"))?;
		}
	}
	let out = child
		.wait_with_output()
		.map_err(|e| format!("git wait failed: {e}"))?;
	if !out.status.success() {
		let err = String::from_utf8_lossy(&out.stderr);
		return Err(format!("git {:?} failed: {}", args, err.trim()));
	}
	Ok(out.stdout)
}

/// Probe run for `--verify --quiet`-style ref checks: a clean non-zero exit
/// (empty stderr, e.g. a missing ref) yields empty output; real failures with
/// a message stay errors.
fn run_tolerant(root: &Path, git_dir: Option<&Path>, args: &[&str]) -> Result<Vec<u8>> {
	let mut cmd = git_cmd(root, git_dir, args, None)?;
	cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
	let out = cmd.output().map_err(|e| format!("git spawn failed: {e}"))?;
	if !out.status.success() {
		let err = String::from_utf8_lossy(&out.stderr);
		if err.trim().is_empty() {
			return Ok(Vec::new());
		}
		return Err(format!("git {:?} failed: {}", args, err.trim()));
	}
	Ok(out.stdout)
}

fn lossy(bytes: Vec<u8>) -> String {
	String::from_utf8_lossy(&bytes).into_owned()
}

impl Store {
	fn mutation_lock(&self, scope: &str) -> Result<MutationLock> {
		let key = format!("{:x}", Sha256::digest(scope.as_bytes()));
		let file = std::fs::OpenOptions::new()
			.create(true)
			.truncate(false)
			.read(true)
			.write(true)
			.open(self.dir.join(format!("{key}.lock")))
			.map_err(|e| e.to_string())?;
		file.lock().map_err(|e| format!("gitfs lock: {e}"))?;
		Ok(MutationLock(file))
	}
	pub fn root(&self) -> &Path {
		&self.root
	}

	/// The private session index file, used for all read-tree/write-tree work.
	fn index_file(&self, session: &str) -> PathBuf {
		self.dir.join(format!("{session}.idx"))
	}

	pub fn branch(&self, session: &str) -> String {
		format!("refs/gitfs/{session}")
	}

	pub fn git(&self, args: &[&str]) -> Result<Vec<u8>> {
		run(&self.root, self.git_dir.as_deref(), args, None, None)
	}

	pub fn git_str(&self, args: &[&str]) -> Result<String> {
		String::from_utf8(self.git(args)?).map_err(|e| e.to_string())
	}

	/// The session branch tip, or None when the session has written nothing.
	pub fn tip(&self, session: &str) -> Result<Option<String>> {
		let out = run_tolerant(
			&self.root,
			self.git_dir.as_deref(),
			&["rev-parse", "--verify", "--quiet", &self.branch(session)],
		)?;
		Ok(trimmed_opt(lossy(out)))
	}

	pub fn head(&self) -> Result<Option<String>> {
		let out = run_tolerant(
			&self.root,
			self.git_dir.as_deref(),
			&["rev-parse", "--verify", "--quiet", "HEAD"],
		)?;
		Ok(trimmed_opt(lossy(out)))
	}

	/// Shared commit tip, automatically rebased onto reviewed HEAD when safe.
	pub fn live_tip(&self) -> Result<Option<String>> {
		let live = self.raw_live_tip()?;
		if let (Some(live), Some(head)) = (&live, self.head()?) {
			if !self.is_ancestor(&head, live)? {
				let _repo = self.mutation_lock("repository")?;
				return self.reconcile_live_locked();
			}
		}
		Ok(live)
	}

	fn raw_live_tip(&self) -> Result<Option<String>> {
		let out = run_tolerant(
			&self.root,
			self.git_dir.as_deref(),
			&["rev-parse", "--verify", "--quiet", LIVE_REF],
		)?;
		Ok(trimmed_opt(lossy(out)))
	}

	/// Local writers share a commit chain; if reviewed HEAD advances,
	/// merge non-overlapping head changes into a two-parent live commit.
	/// Reviewed HEAD wins a path whose live edit cannot merge: a live version
	/// trunk already committed is simply superseded, and any other one stays in
	/// the reconcile commit's first parent, which the commit message names.
	/// A conflict therefore never blocks edits to unrelated paths, and the
	/// session branches that made the live edits still hold them.
	fn reconcile_live_locked(&self) -> Result<Option<String>> {
		let Some(live) = self.raw_live_tip()? else {
			return Ok(None);
		};
		let Some(head) = self.head()? else {
			return Ok(Some(live));
		};
		if self.is_ancestor(&head, &live)? {
			return Ok(Some(live));
		}
		let base = self
			.git_str(&["merge-base", &head, &live])
			.map_err(|_| {
				"reviewed HEAD and live history are unrelated; manual reconciliation required"
					.to_owned()
			})?
			.trim()
			.to_owned();
		let changed = self.git(&["diff", "--name-status", "--no-renames", "-z", &base, &live])?;
		let (mut adds, mut removes, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
		for (_, rel) in parse_name_status(&changed) {
			let path = Path::new(&rel);
			let baseline = self.entry_at(&base, path)?;
			let mine = self.entry_at(&live, path)?;
			let current = self.entry_at(&head, path)?;
			let resolved = if current == baseline {
				mine.clone()
			} else if current == mine {
				current.clone()
			} else {
				match self.merge_revision(&base, &head, &rel, mine.as_ref())? {
					Some(blob) => Some((
						mine.as_ref().map_or("100644", |(mode, _)| mode).to_owned(),
						blob,
					)),
					None => {
						if !self.committed_since(&base, &head, &rel, mine.as_ref())? {
							conflicts.push(rel);
						}
						continue; // reviewed HEAD's version stays
					}
				}
			};
			if resolved == current {
				continue;
			}
			match resolved {
				Some((mode, blob)) => adds.push((mode, blob, path.to_path_buf())),
				None => removes.push(path.to_path_buf()),
			}
		}
		let mut message = "gitfs live: reconcile reviewed HEAD".to_owned();
		if !conflicts.is_empty() {
			message.push_str(&format!(
				"\n\nReviewed HEAD kept on conflicting paths; their live versions stay in first parent {live}:\n{}",
				conflicts.join("\n")
			));
		}
		let tree = self.apply("__live__", &self.tree_of(&head)?, &adds, &removes)?;
		let commit = var_str(
			String::from_utf8(self.git(&[
				"commit-tree",
				&tree,
				"-p",
				&live,
				"-p",
				&head,
				"-m",
				&message,
			])?)
			.map_err(|e| e.to_string())?,
		)?;
		if self.head()?.as_deref() != Some(&head) {
			return Err("HEAD changed during live reconciliation; retry".into());
		}
		self.git(&["update-ref", LIVE_REF, &commit, &live])
			.map_err(|e| format!("live reconcile outcome unknown at {commit}: {e}"))?;
		Ok(Some(commit))
	}

	/// Whether some commit in `base..head` held `entry` at `rel`: trunk already
	/// took that live version and moved on, so it is superseded, not a conflict.
	fn committed_since(
		&self,
		base: &str,
		head: &str,
		rel: &str,
		entry: Option<&(String, String)>,
	) -> Result<bool> {
		let Some((_, blob)) = entry else {
			return Ok(false); // a live deletion is never superseded by a trunk edit
		};
		let found = self.git_str(&[
			"log",
			"-1",
			"--format=%H",
			&format!("--find-object={blob}"),
			&format!("{base}..{head}"),
			"--",
			&literal(Path::new(rel)),
		])?;
		Ok(!found.trim().is_empty())
	}

	/// A path changed on the shared revision, including a published deletion.
	pub fn live_change(&self, rel: &Path) -> Result<Option<Option<Vec<u8>>>> {
		let Some(live) = self.live_tip()? else {
			return Ok(None);
		};
		let current = self.entry_at(&live, rel)?;
		let trunk = match self.head()? {
			Some(head) => self.entry_at(&head, rel)?,
			None => None,
		};
		if current == trunk {
			if current.is_none() && self.explicit_live_deletion(rel)? {
				return Ok(Some(None));
			}
			return Ok(None);
		}
		match current {
			Some((_, blob)) => Ok(Some(Some(self.git(&["cat-file", "blob", &blob])?))),
			None => Ok(Some(None)),
		}
	}

	/// An explicit deletion remains visible even when deleting a newly added
	/// file makes its tree identical to HEAD. A rollback deletion is not explicit.
	fn explicit_live_deletion(&self, rel: &Path) -> Result<bool> {
		if self.live_tip()?.is_none() {
			return Ok(false);
		}
		let value = self.git_str(&[
			"log",
			"-1",
			"--format=%(trailers:key=Gitfs-Deletion,valueonly)",
			LIVE_REF,
			"--",
			&literal(rel),
		])?;
		Ok(value.trim() == "true")
	}

	/// File content on the shared revision, or on HEAD before its first edit.
	pub fn live_content(&self, rel: &Path) -> Result<Option<Vec<u8>>> {
		let Some(rev) = self.live_tip()?.or(self.head()?) else {
			return Ok(None);
		};
		match self.entry_at(&rev, rel)? {
			Some((_, blob)) => self.git(&["cat-file", "blob", &blob]).map(Some),
			None => Ok(None),
		}
	}

	/// The latest published private tip for this session is recorded in the
	/// shared commit. A private write made since then is not silently replaced
	/// by an edit of the shared view.
	pub fn unpublished_session_path(&self, session: &str, rel: &Path) -> Result<bool> {
		let Some(live) = self.live_tip()? else {
			return Ok(false);
		};
		let history = self.git_str(&["log", "--max-count=256", "--format=%B%x00", &live])?;
		for message in history.split('\0') {
			let lines = message.lines().collect::<Vec<_>>();
			if !lines
				.iter()
				.any(|line| *line == format!("Gitfs-Session: {session}"))
			{
				continue;
			}
			let Some(overlay) = lines
				.iter()
				.find_map(|line| line.strip_prefix("Gitfs-Overlay: "))
			else {
				return Ok(false);
			};
			if overlay.is_empty() || self.tip(session)?.as_deref() == Some(overlay) {
				return Ok(false);
			}
			return self
				.git(&[
					"diff",
					"--name-only",
					overlay,
					&self.branch(session),
					"--",
					&literal(rel),
				])
				.map(|paths| !paths.is_empty());
		}
		Ok(false)
	}

	/// Paths changed by the shared live revision relative to reviewed HEAD.
	pub fn live_paths(&self) -> Result<Vec<Owned>> {
		let Some(live) = self.live_tip()? else {
			return Ok(Vec::new());
		};
		let base = match self.head()? {
			Some(head) => head,
			None => self.empty_tree()?,
		};
		let changed = self.git(&["diff", "--name-status", "--no-renames", "-z", &base, &live])?;
		let mut paths = parse_name_status(&changed);
		// A newly added file that was subsequently deleted has no net tree
		// diff against HEAD. Preserve its explicit tombstone for peer searches.
		let deleted = self.git(&[
			"log",
			"--format=",
			"--name-only",
			"--diff-filter=D",
			"--no-renames",
			"-z",
			&live,
		])?;
		for name in deleted.split(|byte| *byte == 0) {
			let name = String::from_utf8_lossy(name);
			let name = name.trim_start_matches(['\r', '\n']);
			if name.is_empty() || paths.iter().any(|(_, path)| path == name) {
				continue;
			}
			if self.explicit_live_deletion(Path::new(name))? {
				paths.push(('D', name.to_owned()));
			}
		}
		paths.sort_by(|a, b| a.1.cmp(&b.1));
		Ok(paths)
	}

	/// Create a clean, detached worktree pinned to one exact shared revision.
	/// Builds in it never touch another agent's checkout or staging area.
	pub fn create_snapshot(&self, session: &str) -> Result<Value> {
		if self.git_dir.is_some() {
			return Err("snapshots require a project Git repository".into());
		}
		let _repo = self.mutation_lock("repository")?;
		let revision = self
			.reconcile_live_locked()?
			.or(self.head()?)
			.ok_or("snapshot requires a published live revision or HEAD")?;
		if let Some(head) = self.head()? {
			if !self.is_ancestor(&head, &revision)? {
				return Err(
					"live revision diverged from HEAD; reconcile before creating a snapshot".into(),
				);
			}
		}
		let snapshots = self.dir.join("snapshots");
		std::fs::create_dir_all(&snapshots).map_err(|e| format!("snapshot directory: {e}"))?;
		let root = self.root.canonicalize().map_err(|e| e.to_string())?;
		let snapshots = snapshots.canonicalize().map_err(|e| e.to_string())?;
		if !snapshots.starts_with(root) {
			return Err("snapshot directory escapes the repository".into());
		}
		let owner = format!("{:x}", Sha256::digest(session.as_bytes()));
		let prefix = format!("live-{}-", &owner[..12]);
		let nonce = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map_err(|e| e.to_string())?
			.as_nanos();
		let id = format!(
			"{prefix}{}-{:x}-{nonce:x}",
			&revision[..12],
			std::process::id()
		);
		let path = snapshots.join(&id);
		if path.exists() {
			return Err("snapshot ID collision; retry".into());
		}
		let git_path = crate::hook::git_command_path(&path)?;
		self.git(&["worktree", "add", "--detach", "--", &git_path, &revision])?;
		Ok(json!({"id":id,"path":git_path,"revision":revision,"head":self.head()?}))
	}

	/// Remove only the caller's generated worktree. Git refuses a dirty
	/// checkout; never force-remove another agent's files.
	pub fn release_snapshot(&self, session: &str, id: &str) -> Result<Value> {
		if self.git_dir.is_some() {
			return Err("snapshots require a project Git repository".into());
		}
		let owner = format!("{:x}", Sha256::digest(session.as_bytes()));
		let prefix = format!("live-{}-", &owner[..12]);
		let suffix = id
			.strip_prefix(&prefix)
			.ok_or("snapshot ID does not belong to this session")?;
		if suffix.is_empty()
			|| !suffix
				.bytes()
				.all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
		{
			return Err("invalid snapshot ID".into());
		}
		let _repo = self.mutation_lock("repository")?;
		let snapshots = self
			.dir
			.join("snapshots")
			.canonicalize()
			.map_err(|e| format!("snapshot directory unavailable: {e}"))?;
		let path = snapshots
			.join(id)
			.canonicalize()
			.map_err(|e| format!("snapshot not found: {e}"))?;
		if path.parent() != Some(snapshots.as_path()) {
			return Err("snapshot path escapes its directory".into());
		}
		self.git(&[
			"worktree",
			"remove",
			"--",
			&crate::hook::git_command_path(&path)?,
		])?;
		Ok(json!({"released":id}))
	}

	/// Current branch symbolic name, or None on a detached HEAD.
	pub fn branch_name(&self) -> Result<Option<String>> {
		let out = run_tolerant(
			&self.root,
			self.git_dir.as_deref(),
			&["symbolic-ref", "--quiet", "--short", "HEAD"],
		)?;
		Ok(trimmed_opt(lossy(out)))
	}

	/// Blob sha and mode for a session path, if the session wrote it.
	pub fn blob_for(&self, session: &str, rel: &Path) -> Result<Option<(String, String)>> {
		// A session that has written nothing has no branch ref for ls-tree.
		if self.tip(session)?.is_none() {
			return Ok(None);
		}
		let out = self.git(&["ls-tree", &self.branch(session), "--", &literal(rel)])?;
		let line = std::str::from_utf8(&out).map_err(|e| e.to_string())?;
		let mut cols = line.split_whitespace();
		let mode = cols.next().unwrap_or("100644").to_owned();
		let _kind = cols.next();
		match cols.next().map(str::to_owned) {
			Some(sha) => Ok(Some((sha, mode))),
			None => Ok(None),
		}
	}

	/// The selected Git entry: the session tip when present, otherwise HEAD.
	pub(crate) fn selected_blob(
		&self,
		session: &str,
		rel: &Path,
	) -> Result<Option<(String, String)>> {
		if self.tip(session)?.is_some() {
			return self.blob_for(session, rel);
		}
		match self.head()? {
			Some(head) => Ok(self.entry_at(&head, rel)?.map(|(mode, sha)| (sha, mode))),
			None => Ok(None),
		}
	}

	/// Git mode a session write of `rel` keeps: the session's own blob mode, else
	/// the file on disk (a tracked file before the session's first write, or an
	/// untracked one), else a plain file.
	pub fn mode_for(&self, session: &str, rel: &Path) -> Result<String> {
		if let Some((_, mode)) = self.selected_blob(session, rel)? {
			return Ok(if mode == "100755" {
				mode
			} else {
				"100644".into()
			});
		}
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			if std::fs::metadata(self.root.join(rel))
				.is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
			{
				return Ok("100755".into());
			}
		}
		Ok("100644".into())
	}

	/// Read a session blob. None when the session never wrote that path.
	pub fn read(&self, session: &str, rel: &Path) -> Result<Option<Vec<u8>>> {
		match self.blob_for(session, rel)? {
			Some((sha, _)) => Ok(Some(self.git(&["cat-file", "blob", &sha])?)),
			None => Ok(None),
		}
	}

	/// Trunk's copy of a path (`HEAD:<rel>`), for a read the shared worktree
	/// cannot serve: another session's in-flight move or delete on disk must
	/// not hide from this session what trunk still holds.
	pub fn trunk_read(&self, rel: &Path) -> Result<Option<Vec<u8>>> {
		let Some(head) = self.head()? else {
			return Ok(None);
		};
		Ok(self
			.git(&["cat-file", "blob", &format!("{head}:{}", printable(rel))])
			.ok())
	}

	/// Whether `content` is one of the repository's own revisions of `rel`:
	/// trunk's copy, or the copy on another agent's gitfs session branch. A
	/// shared worktree materializes a sibling's branch content before trunk
	/// moves, so an edit that finds fresher bytes may rebase onto them when
	/// they are such a revision. `exclude` is the asking session, whose own
	/// branch never counts as a newer revision. An unknown hand edit on disk is
	/// not one, and still refuses.
	pub fn known_revision(&self, exclude: &str, rel: &Path, content: &[u8]) -> Result<bool> {
		if self.trunk_read(rel)?.as_deref() == Some(content)
			|| self.live_content(rel)?.as_deref() == Some(content)
		{
			return Ok(true);
		}
		let refs = self.git_str(&["for-each-ref", "--format=%(refname)", "refs/gitfs/"])?;
		for name in refs.lines().take(MAX_KNOWN_REVISIONS) {
			let Some(session) = name.strip_prefix("refs/gitfs/") else {
				continue;
			};
			if session != exclude && self.read(session, rel)?.as_deref() == Some(content) {
				return Ok(true);
			}
		}
		Ok(false)
	}

	/// The merge base of the session branch and trunk, if the session wrote
	/// anything. `None` when there is nothing to base a diff against.
	pub fn merge_base(&self, session: &str) -> Result<Option<String>> {
		match self.tip(session)? {
			None => Ok(None),
			Some(_) => Ok(self.trunk_base(session)),
		}
	}

	/// The merge base of an existing session branch and trunk.
	fn trunk_base(&self, session: &str) -> Option<String> {
		self.git_str(&["merge-base", "HEAD", &self.branch(session)])
			.ok()
			.and_then(trimmed_opt)
	}

	/// Paths the session owns and how they changed relative to trunk at
	/// session start: `(status, path)` with status A/M/D.
	pub fn owned_paths(&self, session: &str) -> Result<Vec<Owned>> {
		let Some(base) = self.change_base(session)? else {
			return Ok(vec![]);
		};
		let out = self.git(&[
			"diff",
			"--name-status",
			"--no-renames",
			"-z",
			&base,
			&self.branch(session),
			"--",
		])?;
		Ok(parse_name_status(&out))
	}

	/// The session view also retains canceled additions and materialized paths
	/// awaiting a rollback. Their net trunk diff can be empty while disk still
	/// needs a deletion or restoration.
	pub(crate) fn session_paths(&self, session: &str) -> Result<Vec<Owned>> {
		let mut paths = self.owned_paths(session)?;
		let Some(tip) = self.tip(session)? else {
			return Ok(paths);
		};
		let base = self.trunk_base(session);
		let mut args = vec![
			"log",
			"--format=",
			"--diff-filter=D",
			"--name-only",
			"--no-renames",
			"-z",
			&tip,
		];
		let exclude = base.map(|base| format!("^{base}"));
		if let Some(exclude) = &exclude {
			args.push(exclude);
		}
		args.push("--");
		for rel in self
			.git(&args)?
			.split(|b| *b == 0)
			.filter(|p| !p.is_empty())
		{
			let rel = std::str::from_utf8(rel).map_err(|_| "session path is not UTF-8")?;
			if !paths.iter().any(|(_, p)| p == rel)
				&& self.blob_for(session, Path::new(rel))?.is_none()
				&& self.explicit_deletion(session, Path::new(rel))?
			{
				paths.push(('D', rel.into()));
			}
		}
		for (rel, entry) in self.load_ledger(session)? {
			if paths.iter().any(|(_, p)| p == &rel) {
				continue;
			}
			let current = self.blob_for(session, Path::new(&rel))?.map(|(sha, _)| sha);
			if current != entry.blob {
				paths.push((if current.is_some() { 'M' } else { 'D' }, rel));
			}
		}
		paths.sort_by(|a, b| a.1.cmp(&b.1));
		Ok(paths)
	}

	/// A rollback removes an overlay entry without requesting deletion of an
	/// untouched disk file. Only the latest deletion's explicit intent persists.
	fn explicit_deletion(&self, session: &str, rel: &Path) -> Result<bool> {
		let branch = self.branch(session);
		let exclude = self.trunk_base(session).map(|base| format!("^{base}"));
		let path = literal(rel);
		let mut args = vec![
			"log",
			"-1",
			"--format=%(trailers:key=Gitfs-Deletion,valueonly)",
			"--diff-filter=D",
			&branch,
		];
		if let Some(exclude) = &exclude {
			args.push(exclude);
		}
		args.extend(["--", &path]);
		Ok(self.git_str(&args)?.trim() == "true")
	}

	fn require_settled_cancellations(&self, session: &str) -> Result<()> {
		let owned = self.owned_paths(session)?;
		for (_, rel) in self.session_paths(session)? {
			if owned.iter().any(|(_, p)| p == &rel) {
				continue;
			}
			let disk = match std::fs::read(self.root.join(&rel)) {
				Ok(bytes) => Some(bytes),
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
				Err(e) => return Err(format!("{rel}: cannot check pending materialization: {e}")),
			};
			if disk != self.read(session, Path::new(&rel))? {
				return Err(format!("{rel}: materialize the pending deletion or rollback before integrating or catching up"));
			}
		}
		Ok(())
	}

	/// The revision a session's changes are diffed against: its merge base
	/// with trunk. A store that is its own git directory has no trunk:
	/// everything on the branch is the session's, so the empty tree is the
	/// base. A project repository without a merge base owns nothing, as
	/// before: an orphan or unborn HEAD there is not "no trunk".
	fn change_base(&self, session: &str) -> Result<Option<String>> {
		if self.tip(session)?.is_none() {
			return Ok(None);
		}
		if let Some(base) = self.trunk_base(session) {
			return Ok(Some(base));
		}
		if self.git_dir.is_none() {
			return Ok(None);
		}
		self.empty_tree().map(Some)
	}

	/// The empty tree's id in this repository's object format.
	fn empty_tree(&self) -> Result<String> {
		let out = run(
			&self.root,
			self.git_dir.as_deref(),
			&["hash-object", "-t", "tree", "--stdin"],
			None,
			Some(b""),
		)?;
		var_str(lossy(out))
	}

	/// How the session changed one path against trunk at session start: `None`
	/// when it did not, `Some(None)` when it deleted it, else its content.
	pub fn session_change(&self, session: &str, rel: &Path) -> Result<Option<Option<Vec<u8>>>> {
		let Some(base) = self.change_base(session)? else {
			return Ok(None);
		};
		let changed = self.git(&[
			"diff",
			"--name-status",
			"--no-renames",
			"-z",
			&base,
			&self.branch(session),
			"--",
			&literal(rel),
		])?;
		if let Some((status, _)) = parse_name_status(&changed).first() {
			return if *status == 'D' {
				Ok(Some(None))
			} else {
				Ok(Some(self.read(session, rel)?))
			};
		}
		let current = self.blob_for(session, rel)?.map(|(sha, _)| sha);
		if let Some(entry) = self.load_ledger(session)?.get(&printable(rel)) {
			if current != entry.blob {
				return Ok(Some(self.read(session, rel)?));
			}
		}
		if current.is_none() && self.explicit_deletion(session, rel)? {
			return Ok(Some(None));
		}
		Ok(None)
	}

	/// The session's own commits, newest first: those on its branch that trunk
	/// does not contain. Bounded to the newest `limit`.
	pub fn session_commits(&self, session: &str, limit: usize) -> Result<Vec<Value>> {
		if self.tip(session)?.is_none() {
			return Ok(vec![]);
		}
		let mut args = vec![
			"log".to_owned(),
			format!("--max-count={limit}"),
			"--format=%H%x00%ct%x00%s".to_owned(),
			self.branch(session),
		];
		if self.head()?.is_some() {
			args.push("--not".into());
			args.push("HEAD".into());
		}
		let args: Vec<&str> = args.iter().map(String::as_str).collect();
		Ok(self
			.git_str(&args)?
			.lines()
			.filter_map(|line| {
				let mut fields = line.splitn(3, '\0');
				let commit = fields.next()?;
				let time = fields.next()?.parse::<u64>().ok()?;
				Some(json!({"commit":commit,"time":time,"subject":fields.next()?}))
			})
			.collect())
	}

	fn is_ancestor(&self, ancestor: &str, of: &str) -> Result<bool> {
		let status = git_cmd(
			&self.root,
			self.git_dir.as_deref(),
			&["merge-base", "--is-ancestor", ancestor, of],
			None,
		)?
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.status()
		.map_err(|e| format!("git spawn failed: {e}"))?;
		match status.code() {
			Some(0) => Ok(true),
			Some(1) => Ok(false),
			_ => Err(format!("cannot compare {ancestor} with {of}")),
		}
	}

	/// Undo one session commit with a new commit per path it changed, each path
	/// back to its content before that commit. Nothing is rewritten, so the
	/// undone change stays in history. Refused when the commit is not the
	/// session's own, or a later session commit changed the same path: roll
	/// that one back first.
	pub(crate) fn rollback_captured(
		&self,
		session: &str,
		commit: &str,
		capture: &mut Capture,
	) -> Result<Vec<String>> {
		let tip = self
			.tip(session)?
			.ok_or("session has no branch to roll back")?;
		if commit.is_empty() || commit.starts_with('-') {
			return Err(format!("rollback: invalid commit {commit}"));
		}
		let commit = var_str(
			self.git_str(&["rev-parse", "--verify", &format!("{commit}^{{commit}}")])
				.map_err(|_| format!("rollback: unknown commit {commit}"))?,
		)?;
		let trunk = self.head()?;
		if !self.is_ancestor(&commit, &tip)?
			|| trunk
				.as_deref()
				.map_or(Ok(false), |h| self.is_ancestor(&commit, h))?
		{
			return Err(format!(
				"rollback: {commit} is not a commit of session {session}"
			));
		}
		// A session without a trunk under it starts at a root commit.
		let parent = match self.tree_of(&format!("{commit}^")) {
			Ok(_) => format!("{commit}^"),
			Err(_) => self.empty_tree()?,
		};
		let changed = self.git(&[
			"diff-tree",
			"-r",
			"--name-status",
			"--no-renames",
			"-z",
			&parent,
			&commit,
		])?;
		let changed = parse_name_status(&changed);
		for (_, rel) in &changed {
			let later = self.git_str(&[
				"rev-list",
				"--max-count=1",
				&format!("{commit}..{tip}"),
				"--",
				&literal(Path::new(rel)),
			])?;
			if !later.trim().is_empty() {
				return Err(format!(
					"rollback: {} changed {rel} later; roll that back first",
					later.trim()
				));
			}
		}
		let mut restored = Vec::new();
		for (_, rel) in changed {
			let path = Path::new(&rel);
			let listing = self.git_str(&["ls-tree", &parent, "--", &literal(path)])?;
			let mut cols = listing.split_whitespace();
			let prior = match (cols.next(), cols.next(), cols.next()) {
				(Some(mode), Some("blob"), Some(sha)) => {
					Some((self.git(&["cat-file", "blob", sha])?, mode.to_owned()))
				}
				_ => None,
			};
			self.commit_path_captured(
				session,
				path,
				prior
					.as_ref()
					.map(|(bytes, mode)| (bytes.as_slice(), mode.as_str())),
				capture,
				Operation::Edit,
				&format!("rollback {} of {commit}", printable(path)),
			)?;
			restored.push(rel);
		}
		Ok(restored)
	}

	/// Put one path back to its content at session start (absent if trunk had
	/// none then), so the session no longer owns it. Unlike a commit rollback
	/// this ignores later commits on the path: it is how a stray write leaves
	/// a session whose commit chain on that path cannot be unwound.
	pub(crate) fn rollback_path_captured(
		&self,
		session: &str,
		rel: &str,
		capture: &mut Capture,
	) -> Result<()> {
		let path = Path::new(rel);
		// Pending materialization can retain a tombstone after its tree change was undone.
		if !self
			.owned_paths(session)?
			.iter()
			.any(|(_, owned)| Path::new(owned) == path)
		{
			return Err(format!("rollback: session {session} does not change {rel}"));
		}
		// No merge base here means no trunk: the path did not exist before.
		let listing = match self.merge_base(session)? {
			Some(base) => self.git_str(&["ls-tree", &base, "--", &literal(path)])?,
			None => String::new(),
		};
		let mut cols = listing.split_whitespace();
		let prior = match (cols.next(), cols.next(), cols.next()) {
			(Some(mode), Some("blob"), Some(sha)) => {
				Some((self.git(&["cat-file", "blob", sha])?, mode.to_owned()))
			}
			_ => None,
		};
		self.commit_path_captured(
			session,
			path,
			prior
				.as_ref()
				.map(|(bytes, mode)| (bytes.as_slice(), mode.as_str())),
			capture,
			Operation::Edit,
			&format!("rollback {} to session start", printable(path)),
		)?;
		Ok(())
	}

	/// Move a session onto trunk. Each owned path trunk did not touch since the
	/// session's base is replayed onto the current head; a path trunk moved to
	/// exactly the session's content (a change trunk already holds, e.g. an
	/// earlier integrate) is dropped; a path both sides changed is merged
	/// three-way and replayed when the changes do not overlap (another
	/// agent's append to the same file lands with yours). Only overlapping
	/// changes refuse the whole catchup, naming every such path and the way
	/// out, with nothing written. When nothing is left to replay the session
	/// ref is deleted and the session starts empty at trunk.
	pub(crate) fn catchup_captured(&self, session: &str, capture: &mut Capture) -> Result<Value> {
		capture.check_session(session)?;
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		self.require_settled_cancellations(session)?;
		let head = self.head()?.ok_or("catchup: trunk has no commit yet")?;
		let at_trunk = |base: Option<&str>| json!({"caught_up":true,"base":base,"head":head,"replayed":[],"merged":[],"dropped":[],"ref_deleted":false});
		let Some(tip) = self.tip(session)? else {
			return Ok(at_trunk(None));
		};
		let base =
			self.merge_base(session)?.ok_or_else(|| {
				format!("catchup: refs/gitfs/{session} shares no history with trunk; start a fresh session")
			})?;
		if base == head {
			return Ok(at_trunk(Some(&base)));
		}
		let entry = |rev: &str, rel: &str| self.entry_at(rev, Path::new(rel));
		let (mut replay, mut merged, mut dropped, mut conflicts) =
			(Vec::new(), Vec::new(), Vec::new(), Vec::new());
		for (_, rel) in self.owned_paths(session)? {
			let moved = self.git(&[
				"diff",
				"--name-only",
				"--no-renames",
				"-z",
				&base,
				&head,
				"--",
				&literal(Path::new(&rel)),
			])?;
			let mine = entry(&tip, &rel)?;
			let theirs = entry(&head, &rel)?;
			if moved.is_empty() {
				replay.push((rel, mine));
				continue;
			}
			if mine == theirs {
				dropped.push(rel);
				continue;
			}
			match self.merge_revision(&base, &head, &rel, mine.as_ref())? {
				Some(blob) if theirs.as_ref().is_some_and(|(_, sha)| sha == &blob) => {
					dropped.push(rel)
				}
				Some(blob) => {
					let mode = mine
						.map(|(mode, _)| mode)
						.unwrap_or_else(|| "100644".into());
					replay.push((rel.clone(), Some((mode, blob))));
					merged.push(rel);
				}
				None => conflicts.push(rel),
			}
		}
		if !conflicts.is_empty() {
			let merged = if merged.is_empty() {
				String::new()
			} else {
				format!(" ({} merged cleanly: {})", merged.len(), merged.join(", "))
			};
			return Err(format!(
				"catchup: trunk changed {} since this session's base {base} and the two revisions edit the same lines{merged}; for each: tool.gitfs.rollback {{path}}, tool.read it again, rewrite on top of trunk, then catchup",
				conflicts.join(", ")
			));
		}
		let branch = self.branch(session);
		let name = format!("{session}.reads.json");
		let mut reads: HashMap<String, String> = self.load_json(&name)?;
		for rel in &dropped {
			reads.remove(rel);
		}
		if replay.is_empty() {
			self.git(&["update-ref", "-d", &branch, &tip])?;
			reads.clear();
			self.save_json(&name, &reads)?;
			return Ok(
				json!({"caught_up":true,"base":base,"head":head,"replayed":[],"merged":[],"dropped":dropped,"ref_deleted":true}),
			);
		}
		let (mut parent, mut tree) = (head.clone(), self.tree_of(&head)?);
		for (rel, mine) in &replay {
			let path = PathBuf::from(rel);
			let next = match mine {
				Some((mode, sha)) => {
					self.apply(session, &tree, &[(mode.clone(), sha.clone(), path)], &[])?
				}
				None => self.apply(session, &tree, &[], &[path])?,
			};
			if next == tree {
				continue;
			}
			parent = self.commit_tree(
				&next,
				Some(&parent),
				&format!("gitfs: catchup replay {rel}\n\nGitfs-Session: {session}"),
			)?;
			tree = next;
			reads.insert(rel.clone(), head.clone());
		}
		if let Err(error) = self.update_branch(&branch, &parent, Some(&tip)) {
			capture.uncertain();
			return Err(format!("catchup: moving {branch} to {parent} failed: {error}; tool.gitfs.ls shows where it stands"));
		}
		self.save_json(&name, &reads)?;
		let replayed: Vec<&String> = replay.iter().map(|(rel, _)| rel).collect();
		Ok(
			json!({"caught_up":true,"base":base,"head":head,"tip":parent,"replayed":replayed,"merged":merged,"dropped":dropped,"ref_deleted":false,"worktree":"materialize again to copy the caught-up paths in"}),
		)
	}

	/// The tree sha a ref resolves to (its commit's tree), for no-op checks.
	fn tree_of(&self, rev: &str) -> Result<String> {
		var_str(self.git_str(&[
			"rev-parse",
			"--verify",
			"--quiet",
			&format!("{rev}^{{tree}}"),
		])?)
	}

	/// A session branch lives in this repository only: a path under a gitlink
	/// (a submodule) has no tree here to commit into, so say which submodule
	/// and the way round instead of git's "appears as both a file and as a
	/// directory".
	fn refuse_submodule(&self, base_tree: &str, rel: &Path) -> Result<()> {
		if base_tree.is_empty() {
			return Ok(());
		}
		let ancestors: Vec<String> = rel
			.ancestors()
			.skip(1)
			.filter(|a| !a.as_os_str().is_empty())
			.map(printable)
			.collect();
		if ancestors.is_empty() {
			return Ok(());
		}
		let mut args = vec!["ls-tree", base_tree, "--"];
		args.extend(ancestors.iter().map(String::as_str));
		let listing = self.git_str(&args)?;
		// ls-tree descends into a listed directory to reach a deeper
		// ancestor and then prints its siblings too, so only a gitlink that
		// is itself one of the ancestors counts.
		match listing
			.lines()
			.filter(|line| line.starts_with("160000 "))
			.filter_map(|line| line.split('\t').nth(1))
			.find(|path| ancestors.iter().any(|a| a == path))
		{
			Some(sub) => Err(format!(
				"{} is inside the git submodule `{sub}`: fs sessions branch this repository only; edit it in the `{sub}` checkout, commit and push there, then bump the gitlink here",
				printable(rel)
			)),
			None => Ok(()),
		}
	}

	fn hash_object(&self, bytes: &[u8]) -> Result<String> {
		let out = run(
			&self.root,
			self.git_dir.as_deref(),
			&["hash-object", "-w", "--stdin"],
			None,
			Some(bytes),
		)?;
		var_str(String::from_utf8(out).map_err(|e| e.to_string())?)
	}

	/// Build a tree in the private index from a base tree plus additions and
	/// removals, then write it out.
	fn apply(
		&self,
		session: &str,
		base_tree: &str,
		adds: &[(String, String, PathBuf)], // (mode, blob sha, rel)
		removes: &[PathBuf],
	) -> Result<String> {
		let index = self.index_file(session);
		let _ = std::fs::remove_file(&index);
		if base_tree.is_empty() {
			run(
				&self.root,
				self.git_dir.as_deref(),
				&["read-tree", "--empty"],
				Some(&index),
				None,
			)?;
		} else {
			run(
				&self.root,
				self.git_dir.as_deref(),
				&["read-tree", base_tree],
				Some(&index),
				None,
			)?;
		}
		for (mode, blob, rel) in adds {
			run(
				&self.root,
				self.git_dir.as_deref(),
				&[
					"update-index",
					"--add",
					"--cacheinfo",
					mode,
					blob,
					&printable(rel),
				],
				Some(&index),
				None,
			)?;
		}
		for rel in removes {
			run(
				&self.root,
				self.git_dir.as_deref(),
				&["update-index", "--force-remove", "--", &printable(rel)],
				Some(&index),
				None,
			)?;
		}
		let out = run(
			&self.root,
			self.git_dir.as_deref(),
			&["write-tree"],
			Some(&index),
			None,
		)?;
		var_str(String::from_utf8(out).map_err(|e| e.to_string())?)
	}

	fn commit_tree(&self, tree: &str, parent: Option<&str>, message: &str) -> Result<String> {
		let mut args = vec!["commit-tree", tree];
		if let Some(parent) = parent {
			args.push("-p");
			args.push(parent);
		}
		args.push("-m");
		args.push(message);
		var_str(String::from_utf8(self.git(&args)?).map_err(|e| e.to_string())?)
	}

	fn update_branch(&self, ref_: &str, commit: &str, expect: Option<&str>) -> Result<()> {
		let mut args = vec!["update-ref", ref_, commit];
		if let Some(expect) = expect {
			args.push(expect);
		}
		self.git(&args).map(|_| ())
	}

	/// Publish one agent edit to the shared revision without touching HEAD,
	/// the common index or worktree. The repository lock serializes local
	/// writers; update-ref compares the old tip for external writers.
	pub fn publish_live(
		&self,
		session: &str,
		rel: &Path,
		before: Option<&[u8]>,
		after: Option<&[u8]>,
		mode: &str,
	) -> Result<Option<String>> {
		self.publish_live_with_intent(session, rel, before, after, mode, after.is_none())
	}

	/// Rollback deletions restore the baseline; they are not explicit tombstones.
	pub fn publish_live_rollback(
		&self,
		session: &str,
		rel: &Path,
		before: Option<&[u8]>,
		after: Option<&[u8]>,
		mode: &str,
	) -> Result<Option<String>> {
		self.publish_live_with_intent(session, rel, before, after, mode, false)
	}

	fn publish_live_with_intent(
		&self,
		session: &str,
		rel: &Path,
		before: Option<&[u8]>,
		after: Option<&[u8]>,
		mode: &str,
		explicit_delete: bool,
	) -> Result<Option<String>> {
		let _repository = self.mutation_lock("repository")?;
		let previous = self.reconcile_live_locked()?;
		let parent = previous.clone().or(self.head()?);
		let tree = match parent.as_deref() {
			Some(parent) => self.tree_of(parent)?,
			None => self.empty_tree()?,
		};
		self.refuse_submodule(&tree, rel)?;
		let entry = self.entry_at(&tree, rel)?;
		let current = match &entry {
			Some((_, blob)) => Some(self.git(&["cat-file", "blob", blob])?),
			None => None,
		};
		let standalone = self.head()?.is_none()
			&& current.is_none()
			&& before.is_some()
			&& self.live_change(rel)?.is_none();
		let sibling = if previous.is_none() {
			match before {
				Some(bytes) => {
					self.known_revision(session, rel, bytes)? && current == self.trunk_read(rel)?
				}
				None => false,
			}
		} else {
			false
		};
		let proposed = if current.as_deref() == before || standalone || sibling {
			after.map(|bytes| bytes.to_vec())
		} else {
			match (before, after, current.as_deref()) {
				(Some(base), Some(ours), Some(theirs)) => crate::merge::merge3(base, ours, theirs)?
					.ok_or_else(|| {
						format!(
							"live conflict in {}; read the latest revision",
							rel.display()
						)
					})
					.map(Some)?,
				(None, Some(ours), Some(theirs)) if ours == theirs => Some(ours.to_vec()),
				(Some(_), None, None) => None,
				_ => {
					return Err(format!(
						"live conflict in {}; read the latest revision",
						rel.display()
					))
				}
			}
		};
		if proposed.as_deref() == current.as_deref() {
			return Ok(previous);
		}
		let updated = match proposed {
			Some(bytes) => {
				let blob = self.hash_object(&bytes)?;
				let mode = entry
					.as_ref()
					.map_or(mode, |(existing, _)| existing.as_str());
				self.apply(
					"__live__",
					&tree,
					&[(mode.into(), blob, rel.to_path_buf())],
					&[],
				)?
			}
			None => self.apply("__live__", &tree, &[], &[rel.to_path_buf()])?,
		};
		let deletion = if explicit_delete {
			"\nGitfs-Deletion: true"
		} else {
			""
		};
		let commit = self.commit_tree(
			&updated,
			parent.as_deref(),
			&format!(
				"gitfs live: {}\n\nGitfs-Session: {session}\nGitfs-Overlay: {}{deletion}",
				printable(rel),
				self.tip(session)?.unwrap_or_default()
			),
		)?;
		self.git(&[
			"update-ref",
			LIVE_REF,
			&commit,
			previous
				.as_deref()
				.unwrap_or("0000000000000000000000000000000000000000"),
		])
		.map_err(|error| {
			format!(
				"live publication outcome unknown for {commit}: {error}; reconcile before retry"
			)
		})?;
		Ok(Some(commit))
	}

	/// Commit one path into the session branch. Returns the new tip; `None`
	/// when the content was already there (no new commit).
	#[allow(dead_code)] // Preserve the original store convenience API.
	pub fn write_blob(
		&self,
		session: &str,
		rel: &Path,
		bytes: &[u8],
		mode: &str,
	) -> Result<Option<String>> {
		self.write_blob_captured(
			session,
			rel,
			bytes,
			mode,
			&mut Capture::default(),
			Operation::Write,
		)
	}
	pub(crate) fn write_blob_captured(
		&self,
		session: &str,
		rel: &Path,
		bytes: &[u8],
		mode: &str,
		capture: &mut Capture,
		op: Operation,
	) -> Result<Option<String>> {
		self.commit_path_captured(
			session,
			rel,
			Some((bytes, mode)),
			capture,
			op,
			&format!("save {}", printable(rel)),
		)
	}

	/// Commit one path's new content, or its removal when `content` is `None`,
	/// onto the session branch. `None` back when the tree would not change.
	pub(crate) fn delete_blob_captured(
		&self,
		session: &str,
		rel: &Path,
		capture: &mut Capture,
		op: Operation,
	) -> Result<Option<String>> {
		self.commit_path_captured(
			session,
			rel,
			None,
			capture,
			op,
			&format!("delete {}", printable(rel)),
		)
	}

	pub(crate) fn delete_blob_checked(
		&self,
		session: &str,
		rel: &Path,
		expected: [u8; 32],
		capture: &mut Capture,
	) -> Result<Option<String>> {
		capture.check_session(session)?;
		let _guard = self.mutation_lock(&format!("session:{session}"))?;
		self.commit_path_locked_checked(
			session,
			rel,
			None,
			capture,
			Operation::Delete,
			&format!("delete {}", printable(rel)),
			Some(expected),
		)
	}

	fn commit_path_captured(
		&self,
		session: &str,
		rel: &Path,
		content: Option<(&[u8], &str)>,
		capture: &mut Capture,
		op: Operation,
		subject: &str,
	) -> Result<Option<String>> {
		capture.check_session(session)?;
		let _g = self.mutation_lock(&format!("session:{session}"))?;
		self.commit_path_locked(session, rel, content, capture, op, subject)
	}

	/// `commit_path_captured` under a session lock the caller already holds.
	/// `materialize` commits a clean three-way merge while its own mutation
	/// locks are held, so it calls this body instead of deadlocking on a
	/// second lock of the same session.
	fn commit_path_locked(
		&self,
		session: &str,
		rel: &Path,
		content: Option<(&[u8], &str)>,
		capture: &mut Capture,
		op: Operation,
		subject: &str,
	) -> Result<Option<String>> {
		self.commit_path_locked_checked(session, rel, content, capture, op, subject, None)
	}

	#[allow(clippy::too_many_arguments)]
	fn commit_path_locked_checked(
		&self,
		session: &str,
		rel: &Path,
		content: Option<(&[u8], &str)>,
		capture: &mut Capture,
		op: Operation,
		subject: &str,
		expected: Option<[u8; 32]>,
	) -> Result<Option<String>> {
		let branch = self.branch(session);
		let prev = self.tip(session)?;
		let head = self.head()?;
		let base_tree = match &prev {
			Some(_) => self.tree_of(&branch)?,
			None => match &head {
				Some(h) => self.tree_of(h)?,
				None => String::new(), // unborn repo
			},
		};
		self.refuse_submodule(&base_tree, rel)?;
		if let Some(expected) = expected {
			let (_, sha) = self
				.entry_at(&base_tree, rel)?
				.ok_or("file disappeared since it was read; reread it")?;
			let actual: [u8; 32] = Sha256::digest(self.git(&["cat-file", "blob", &sha])?).into();
			if actual != expected {
				return Err("file changed since it was read; reread it".into());
			}
		}
		let claimed = printable(rel);
		let fresh_claim = !self
			.owned_paths(session)?
			.iter()
			.any(|(_, owned)| owned == &claimed);
		let deletion_intent =
			content.is_none() && matches!(&op, Operation::Delete | Operation::Capture);
		let mut evidence = capture.reserve(self, rel, op, Storage::GitfsOverlay)?;
		if let Some(e) = &mut evidence {
			e.overlay_commit.before = prev.clone();
			if !base_tree.is_empty() {
				let output = self.git(&["ls-tree", &base_tree, "--", &literal(rel)])?;
				if let Some(sha) = std::str::from_utf8(&output)
					.map_err(|e| e.to_string())?
					.split_whitespace()
					.nth(2)
				{
					e.before = Version::GitObject { value: sha.into() };
				}
			}
		}
		let (tree, blob) = match content {
			Some((bytes, mode)) => {
				let blob = self.hash_object(bytes)?;
				let adds = [(mode.to_owned(), blob.clone(), rel.to_path_buf())];
				(self.apply(session, &base_tree, &adds, &[])?, Some(blob))
			}
			None => (
				self.apply(session, &base_tree, &[], &[rel.to_path_buf()])?,
				None,
			),
		};
		// Parent: the session tip, else the trunk head, else none (unborn repo).
		let parent = prev.clone().or_else(|| head.clone());
		if tree == base_tree {
			return Ok(None); // content already at the tip
		}
		let deletion = if deletion_intent {
			"\nGitfs-Deletion: true"
		} else {
			""
		};
		let commit = self.commit_tree(
			&tree,
			parent.as_deref(),
			&format!("gitfs: {subject}\n\nGitfs-Session: {session}{deletion}"),
		)?;
		if let Some(e) = &mut evidence {
			if let Some(value) = blob {
				e.after = Version::GitObject { value };
			}
			e.overlay_commit.after = Some(commit.clone());
			e.validate(session)?;
		}
		if let Err(error) = self.update_branch(&branch, &commit, prev.as_deref()) {
			capture.uncertain();
			return Err(format!("overlay publication outcome unknown for {commit}: {error}; do not automatically retry"));
		}
		if fresh_claim {
			self.record_read(session, &claimed, head.as_deref())?;
		}
		capture.keep(evidence);
		Ok(Some(commit))
	}

	/// Merge one materialize conflict. The shared worktree holds a revision
	/// another agent put there; when that revision was committed (trunk or a
	/// gitfs session branch) it is safe to merge with this session's copy
	/// three-way. The merged bytes are committed onto this session branch, so
	/// branch and worktree agree after the write. `None` leaves the caller's
	/// refusal in place for an unknown hand edit, an overlapping change, or no
	/// common base.
	fn merge_disk_revision(
		&self,
		session: &str,
		rel: &str,
		ours: &[u8],
		mode: &str,
		disk: &[u8],
	) -> Result<Option<Vec<u8>>> {
		let path = Path::new(rel);
		if !self.known_revision(session, path, disk)? {
			return Ok(None);
		}
		// The trunk snapshot both branches diverged from. Our last
		// materialize ledger entry is not a base: it is our own content, and
		// merging onto it would quietly drop this session's change.
		let base = match self.merge_base(session)? {
			Some(base) => self
				.entry_at(&base, path)?
				.and_then(|(_, sha)| self.git(&["cat-file", "blob", &sha]).ok()),
			None => None,
		};
		let Some(base) = base else {
			return Ok(None);
		};
		let Some(merged) = crate::merge::merge3(&base, ours, disk)? else {
			return Ok(None);
		};
		self.commit_path_locked(
			session,
			path,
			Some((&merged, mode)),
			&mut Capture::default(),
			Operation::Materialize,
			&format!("merge the shared worktree copy of {rel}"),
		)?;
		Ok(Some(merged))
	}

	/// Copy the session's owned paths into the working tree, refusing to
	/// overwrite on-disk files newer than the copy it would install. Returns
	/// what landed, what conflicted and why. `paths` narrows the operation.
	#[cfg(test)]
	pub fn materialize(
		&self,
		session: &str,
		paths: Option<&[String]>,
		force: bool,
	) -> Result<Materialize> {
		self.materialize_captured(session, paths, force, &mut Capture::default())
	}
	pub(crate) fn materialize_captured(
		&self,
		session: &str,
		paths: Option<&[String]>,
		force: bool,
		capture: &mut Capture,
	) -> Result<Materialize> {
		capture.check_session(session)?;
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		let mut ledger = self.load_ledger(session)?;
		let mut result = Materialize {
			applied: vec![],
			merged: vec![],
			conflicts: vec![],
			errors: vec![],
		};
		// A session with no commit yet owns nothing: the repository worktree
		// already is its branch, so there is nothing to copy and nothing to refuse.
		let Some(tip) = self.tip(session)? else {
			return Ok(result);
		};
		// Commit times are whole seconds; disk times are milliseconds.
		let tip_ts = seconds_end_ms(self.tip_time(&tip)?);
		for (status, rel) in self.session_paths(session)? {
			if let Some(allowed) = paths {
				if !allowed.iter().any(|p| p == &rel) {
					continue;
				}
			}
			let disk = self.root.join(&rel);
			let disk_mtime = fs_mtime(&disk);
			match status {
				'M' | 'A' => {
					let (mut sha, mut mode) = match self.blob_for(session, Path::new(&rel)) {
						Ok(Some(v)) => v,
						Ok(None) => continue,
						Err(e) => {
							result.errors.push(format!("{rel}: {e}"));
							continue;
						}
					};
					let mut content = match self.read(session, Path::new(&rel)) {
						Ok(Some(v)) => v,
						Ok(None) => continue,
						Err(e) => {
							result.errors.push(format!("{rel}: {e}"));
							continue;
						}
					};
					// Disk already holds this content (a native edit the post-edit
					// hook mirrored, or an earlier materialize): nothing to copy
					// and nothing to refuse.
					if !force && std::fs::read(&disk).ok().as_deref() == Some(content.as_slice()) {
						ledger.insert(
							rel.clone(),
							LedgerEntry {
								blob: Some(sha),
								mtime: disk_mtime.unwrap_or_else(now_ms),
							},
						);
						continue;
					}
					let ts = self
						.path_time(session, &rel)
						.map(seconds_end_ms)
						.unwrap_or(tip_ts);
					let entry = ledger.get(&rel).cloned();
					let conflict = match &entry {
						Some(e) if !force => {
							let expected = e
								.blob
								.as_ref()
								.map(|sha| self.git(&["cat-file", "blob", sha]))
								.transpose()?;
							let actual = match std::fs::read(&disk) {
								Ok(bytes) => Some(bytes),
								Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
								Err(e) => {
									result
										.errors
										.push(format!("{rel}: materialization check failed: {e}"));
									continue;
								}
							};
							(actual != expected).then(|| {
								"file modified or removed on disk since it was materialized"
									.to_string()
							})
						}
						None => {
							if let Some(dm) = disk_mtime {
								if dm > ts && !force {
									Some("on-disk file is newer than the gitfs copy".to_string())
								} else {
									None
								}
							} else {
								None
							}
						}
						_ => None,
					};
					if let Some(reason) = conflict {
						let disk_bytes = if disk_mtime.is_some() {
							std::fs::read(&disk).ok()
						} else {
							None
						};
						let merged = match disk_bytes.as_deref() {
							Some(theirs) => {
								self.merge_disk_revision(session, &rel, &content, &mode, theirs)
							}
							None => Ok(None),
						};
						match merged {
							Ok(Some(bytes)) => {
								content = bytes;
								if let Ok(Some((blob, merged_mode))) =
									self.blob_for(session, Path::new(&rel))
								{
									sha = blob;
									mode = merged_mode;
								}
								result.merged.push(rel.clone());
							}
							Ok(None) => {
								result.conflicts.push((rel.clone(), reason));
								continue;
							}
							Err(e) => {
								result
									.errors
									.push(format!("{rel}: merge of the on-disk copy failed: {e}"));
								continue;
							}
						}
					}
					let mut evidence = match capture.reserve(
						self,
						Path::new(&rel),
						Operation::Materialize,
						Storage::Direct,
					) {
						Ok(v) => v,
						Err(e) => {
							result.errors.push(format!("{rel}: {e}"));
							continue;
						}
					};
					let before = if evidence.is_some() {
						provenance::preimage(&disk)
					} else {
						Ok(Version::Absent {})
					};
					match self.write_disk(&disk, &content, &mode) {
						Ok(Some(e)) => result.errors.push(format!("{rel}: content published; {e}")),
						Ok(None) => (),
						Err(e) => {
							capture.uncertain();
							result
								.errors
								.push(format!("{rel}: write outcome unknown: {e}"));
							continue;
						}
					}
					if let Some(e) = &mut evidence {
						e.overlay_commit.before = Some(tip.clone());
						e.overlay_commit.after = Some(tip.clone());
						match before {
							Ok(v) => {
								e.before = v;
								e.after = provenance::content_version(&content);
							}
							Err(_) => {
								capture.uncertain();
								evidence = None;
							}
						}
					}
					capture.keep(evidence);
					ledger.insert(
						rel.clone(),
						LedgerEntry {
							blob: Some(sha),
							mtime: fs_mtime(&disk).unwrap_or_else(now_ms),
						},
					);
					result.applied.push(rel.clone());
				}
				'D' => {
					let root = self.root.canonicalize().map_err(|e| e.to_string())?;
					let checked = crate::files::resolve(&root, Path::new(&rel));
					if checked.as_ref().ok() != Some(&root.join(&rel))
						|| std::fs::symlink_metadata(&disk).is_ok_and(|m| !m.is_file())
					{
						result.errors.push(format!(
							"{rel}: deletion refuses a symlink, directory or escaped path"
						));
						continue;
					}
					let disk_bytes = match std::fs::read(&disk) {
						Ok(bytes) => Some(bytes),
						Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
						Err(e) => {
							result
								.errors
								.push(format!("{rel}: deletion check failed: {e}"));
							continue;
						}
					};
					if !force {
						if let Some(bytes) = &disk_bytes {
							match self.deletion_matches(session, &rel, ledger.get(&rel), bytes) {
								Ok(true) => (),
								Ok(false) => {
									result.conflicts.push((
										rel.clone(),
										"file changed or was recreated since the deletion's base"
											.into(),
									));
									continue;
								}
								Err(e) => {
									result.errors.push(format!("{rel}: {e}"));
									continue;
								}
							}
						}
					}
					let mut evidence = match capture.reserve(
						self,
						Path::new(&rel),
						Operation::Materialize,
						Storage::Direct,
					) {
						Ok(v) => v,
						Err(e) => {
							result.errors.push(format!("{rel}: {e}"));
							continue;
						}
					};
					let before = if evidence.is_some() {
						provenance::preimage(&disk)
					} else {
						Ok(Version::Absent {})
					};
					let removed = match std::fs::remove_file(&disk) {
						Ok(()) => true,
						Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
						Err(e) => {
							result.errors.push(format!("{rel}: deletion failed: {e}"));
							continue;
						}
					};
					if removed {
						if let Some(e) = &mut evidence {
							e.overlay_commit.before = Some(tip.clone());
							e.overlay_commit.after = Some(tip.clone());
							match before {
								Ok(v) => e.before = v,
								Err(_) => {
									capture.uncertain();
									evidence = None;
								}
							}
						}
						capture.keep(evidence);
					}

					ledger.insert(
						rel.clone(),
						LedgerEntry {
							blob: None,
							mtime: now_ms(),
						},
					);
					result.applied.push(rel.clone());
				}
				_ => {}
			}
		}
		if let Err(e) = self.save_ledger(session, &ledger) {
			result.errors.push(format!(
				"materialization ledger failed after applied paths: {e}"
			));
		}
		Ok(result)
	}

	// ————— the read set —————

	/// Record the trunk revision a path was claimed at. A write follows a read,
	/// so this is the trunk the session read that path at. Only a fresh claim
	/// records: while the session owns a path its own reads answer from its
	/// branch, so it never sees trunk move that path. Rolling the path back
	/// drops the claim, and the next read and write are against trunk as it
	/// stands then.
	fn record_read(&self, session: &str, rel: &str, head: Option<&str>) -> Result<()> {
		let Some(head) = head else { return Ok(()) };
		let name = format!("{session}.reads.json");
		let mut reads: HashMap<String, String> = self.load_json(&name)?;
		reads.insert(rel.to_owned(), head.to_owned());
		self.save_json(&name, &reads)
	}

	/// The owned paths trunk has changed since this session read them. A
	/// session written before the read set existed falls back to its merge
	/// base, which is the trunk its first write was made against.
	pub fn stale_paths(&self, session: &str, head: &str) -> Result<Vec<String>> {
		let reads: HashMap<String, String> = self.load_json(&format!("{session}.reads.json"))?;
		let fallback = self.merge_base(session)?;
		let mut stale = Vec::new();
		for (_, rel) in self.owned_paths(session)? {
			let Some(base) = reads.get(&rel).cloned().or_else(|| fallback.clone()) else {
				continue;
			};
			if base == head {
				continue;
			}
			let moved = self.git(&[
				"diff",
				"--name-only",
				"--no-renames",
				"-z",
				&base,
				head,
				"--",
				&literal(Path::new(&rel)),
			])?;
			if !moved.is_empty() {
				stale.push(rel);
			}
		}
		Ok(stale)
	}

	/// The trunk commit a integrate with this review revision already published, if
	/// there is one. Git is the receipt store: the review revision and the
	/// consumed session tip are trailers on the commit, so a replayed integrate
	/// answers the same receipt instead of committing a second time.
	pub fn find_integrated(&self, revision: &str) -> Result<Option<Value>> {
		if revision.len() != 64 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Ok(None);
		}
		let attempt = self.integration_attempt(revision)?;
		let reference = attempt
			.as_ref()
			.map(|intent| format!("refs/heads/{}", intent.review.branch))
			.unwrap_or_else(|| "HEAD".to_owned());
		let out = run_tolerant(
			&self.root,
			self.git_dir.as_deref(),
			&[
				"log",
				"-n",
				"1",
				"--fixed-strings",
				&format!("--grep=Gitfs-Review: {revision}"),
				"--format=%H%x00%T%x00%P%x00%s%x00%b",
				&reference,
			],
		)?;
		let Some(text) = trimmed_opt(lossy(out)) else {
			return Ok(None);
		};
		let mut fields = text.split('\0');
		let commit = fields.next().unwrap_or_default().to_owned();
		let tree = fields.next().unwrap_or_default().to_owned();
		let parent = fields
			.next()
			.unwrap_or_default()
			.split_whitespace()
			.next()
			.unwrap_or_default()
			.to_owned();
		let subject = fields.next().unwrap_or_default().to_owned();
		let body = fields.next().unwrap_or_default();
		let trailer = |key: &str| {
			body.lines()
				.find_map(|line| line.trim().strip_prefix(key).map(|v| v.trim().to_owned()))
				.unwrap_or_default()
		};
		if trailer("Gitfs-Review:") != revision {
			return Ok(None);
		}
		if let Some(intent) = &attempt {
			let authors = self.git_str(&["show", "-s", "--format=%an%x00%ae", &commit])?;
			let mut names = authors.trim_end().split('\0');
			let parents = self.git_str(&["rev-list", "--parents", "-n", "1", &commit])?;
			if tree != intent.review.tree
				|| parent != intent.review.head
				|| subject != intent.subject
				|| trailer("Gitfs-Session:") != intent.session
				|| trailer("Gitfs-Consumed:") != intent.review.overlay
				|| names.next() != Some(intent.author_name.as_str())
				|| names.next() != Some(intent.author_email.as_str())
				|| parents.split_whitespace().count() != 2
				|| intent
					.committed
					.as_deref()
					.is_some_and(|known| known != commit)
				|| self
					.git(&["merge-base", "--is-ancestor", &commit, &reference])
					.is_err()
			{
				return Ok(None);
			}
			return Ok(Some(json!({
				"committed":commit,"tree":tree,"parent":parent,"subject":subject,
				"session":intent.session,"consumed":intent.review.overlay,
				"branch":intent.review.branch,"journaled":true,
			})));
		}
		Ok(Some(json!({
			"committed":commit,
			"tree":tree,
			"parent":parent,
			"subject":subject,
			"session":trailer("Gitfs-Session:"),
			"consumed":trailer("Gitfs-Consumed:"),
		})))
	}

	// ————— integrate —————

	/// Build the tree a integrate would commit: the session's owned paths overlaid
	/// onto the current trunk tree. Fails when the session owns nothing or the
	/// overlay would change nothing. The tree is returned so the caller can
	/// diff and gate it before committing.
	pub fn preview_integrate(&self, session: &str) -> Result<(String, Vec<Owned>)> {
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		self.preview_integrate_locked(session)
	}
	fn preview_integrate_locked(&self, session: &str) -> Result<(String, Vec<Owned>)> {
		self.require_settled_cancellations(session)?;
		let Some(head) = self.head()? else {
			return Err("cannot integrate: no HEAD commit".into());
		};
		let owned = self.owned_paths(session)?;
		if owned.is_empty() {
			return Err("nothing to integrate: the session owns no changed paths".into());
		}
		let stale = self.stale_paths(session, &head)?;
		if !stale.is_empty() {
			return Err(format!(
				"stale read revision in {}: trunk moved after this session read them; call tool.gitfs_catchup (clean changes merge, overlapping ones are named there), then ship again",
				stale.join(", ")
			));
		}
		let head_tree = self.tree_of(&head)?;
		let tree = self.overlay_onto(session, &head_tree, &owned)?;
		if tree == head_tree {
			return Err(
				"nothing to integrate: session changes do not survive the current trunk".into(),
			);
		}
		Ok((tree, owned))
	}

	/// The tree a integrate commits: trunk's own tree with the session's owned paths
	/// laid over it. No merge — the read-set guard has already refused every
	/// path trunk moved since the session read it, so there is nothing to
	/// reconcile here.
	fn overlay_onto(&self, session: &str, head_tree: &str, owned: &[Owned]) -> Result<String> {
		let mut adds = Vec::new();
		let mut removes = Vec::new();
		for (status, rel) in owned {
			let path = PathBuf::from(rel);
			if *status == 'D' {
				removes.push(path);
				continue;
			}
			let (blob, mode) = self
				.blob_for(session, &path)?
				.ok_or_else(|| format!("{rel} is owned but missing from the session branch"))?;
			adds.push((mode, blob, path));
		}
		self.apply(session, head_tree, &adds, &removes)
	}

	/// Diff of an overlay tree against the current trunk head (unified 0, like
	/// autogit's staged scan), plus the raw name-status list.
	pub fn tree_diff(&self, tree: &str) -> Result<String> {
		let Some(head) = self.head()? else {
			return Err("no HEAD".into());
		};
		let out = self.git(&[
			"diff",
			"--no-ext-diff",
			"--no-textconv",
			"--no-renames",
			"--unified=0",
			&head,
			tree,
		])?;
		Ok(String::from_utf8_lossy(&out).into_owned())
	}

	/// Commit the overlay tree onto the current branch head with the given
	/// message. Returns the new commit sha.
	#[cfg(test)]
	pub fn commit_integrate(&self, session: &str, tree: &str, message: &str) -> Result<String> {
		let preview = self.reviewed_tree(session)?;
		if preview.tree != tree {
			return Err("reviewed tree changed; preview again".into());
		}
		self.commit_reviewed(
			session,
			&preview,
			message,
			IDENT_NAME,
			IDENT_EMAIL,
			&AtomicBool::new(false),
		)
	}

	fn shared_index(&self) -> Result<PathBuf> {
		let path = self.git_str(&["rev-parse", "--git-path", "index"])?;
		Ok(self.root.join(path.trim()))
	}

	pub(crate) fn repository_identity(&self) -> Result<String> {
		let root = self.root.canonicalize().map_err(|e| e.to_string())?;
		let git_dir = PathBuf::from(self.git_str(&["rev-parse", "--absolute-git-dir"])?.trim())
			.canonicalize()
			.map_err(|e| e.to_string())?;
		let (root_device, root_file) = file_id(&root)?;
		let (git_device, git_file) = file_id(&git_dir)?;
		Ok(format!(
			"{:x}",
			Sha256::digest(
				serde_json::to_vec(&json!([
					root,
					git_dir,
					root_device,
					root_file,
					git_device,
					git_file
				]))
				.unwrap()
			)
		))
	}

	fn reviewed_tree_locked(&self, session: &str) -> Result<ReviewedTree> {
		use std::io::Read;
		let repository = self.repository_identity()?;
		let branch = self.branch_name()?.ok_or("cannot review a detached HEAD")?;
		let head = self.head()?.ok_or("cannot review an unborn HEAD")?;
		let overlay = self.tip(session)?.ok_or("session has no overlay")?;
		let index_revision = match std::fs::File::open(self.shared_index()?) {
			Ok(file) => {
				let mut bytes = Vec::new();
				file.take(64 * 1024 * 1024 + 1)
					.read_to_end(&mut bytes)
					.map_err(|e| e.to_string())?;
				if bytes.len() > 64 * 1024 * 1024 {
					return Err("shared index exceeds review capacity".into());
				}
				format!("{:x}", Sha256::digest(bytes))
			}
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => "absent".into(),
			Err(e) => return Err(format!("shared index unavailable: {e}")),
		};
		let (tree, owned) = self.preview_integrate_locked(session)?;
		if owned.len() > 1024 || owned.iter().any(|(_, p)| p.len() > 4096) {
			return Err("owned paths exceed preview capacity".into());
		}
		if self.head()?.as_deref() != Some(&head)
			|| self.branch_name()?.as_deref() != Some(&branch)
			|| self.tip(session)?.as_deref() != Some(&overlay)
		{
			return Err("repository changed during preview; preview again".into());
		}
		Ok(ReviewedTree {
			repository,
			branch,
			head,
			index_revision,
			overlay,
			tree,
			owned,
		})
	}

	pub fn reviewed_tree(&self, session: &str) -> Result<ReviewedTree> {
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		self.reviewed_tree_locked(session)
	}

	pub fn reviewed_diff(&self, review: &ReviewedTree) -> Result<String> {
		let bytes = self.git(&[
			"diff",
			"--no-ext-diff",
			"--no-textconv",
			"--binary",
			"--no-renames",
			"--unified=0",
			&review.head,
			&review.tree,
		])?;
		String::from_utf8(bytes).map_err(|_| "review diff is not UTF-8".into())
	}

	pub fn commit_reviewed(
		&self,
		session: &str,
		expected: &ReviewedTree,
		message: &str,
		author: &str,
		email: &str,
		cancel: &AtomicBool,
	) -> Result<String> {
		use std::io::Write;
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		// The shared index is read, never locked: the commit comes from the
		// reviewed tree, so holding `.git/index.lock` only made every other
		// agent's git command fail for the length of this publication.
		if cancel.load(Ordering::SeqCst) {
			return Err("cancelled before publication".into());
		}
		if &self.reviewed_tree_locked(session)? != expected {
			return Err("reviewed repository/tree/index changed; preview again".into());
		}
		if cancel.load(Ordering::SeqCst) {
			return Err("cancelled before publication".into());
		}
		let mut child = git_cmd(
			&self.root,
			self.git_dir.as_deref(),
			&["commit-tree", &expected.tree, "-p", &expected.head],
			None,
		)?
		.env("GIT_AUTHOR_NAME", author)
		.env("GIT_AUTHOR_EMAIL", email)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(|e| e.to_string())?;
		child
			.stdin
			.take()
			.unwrap()
			.write_all(message.as_bytes())
			.map_err(|e| e.to_string())?;
		let output = child.wait_with_output().map_err(|e| e.to_string())?;
		if !output.status.success() {
			return Err("commit object creation failed; nothing published".into());
		}
		let commit = String::from_utf8(output.stdout)
			.map_err(|e| e.to_string())?
			.trim()
			.to_owned();
		if cancel.load(Ordering::SeqCst) {
			return Err("cancelled before publication; unreferenced object may remain".into());
		}
		// Prepare locks HEAD, its referent and the overlay. Check the symbolic
		// name only after prepare: Git rejects symref-verify HEAD plus an update
		// to its referent in one transaction (duplicate synthetic HEAD update).
		// A prepared HEAD update prevents concurrent checkout/symbolic-ref changes.
		let mut transaction = git_cmd(
			&self.root,
			self.git_dir.as_deref(),
			&["update-ref", "--stdin"],
			None,
		)?
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.map_err(|e| e.to_string())?;
		let mut dispatched = false;
		let result: Result<()> = (|| {
			use std::io::{BufRead, BufReader};
			let mut input = transaction
				.stdin
				.take()
				.ok_or("missing transaction input")?;
			let mut output = BufReader::new(
				transaction
					.stdout
					.take()
					.ok_or("missing transaction output")?,
			);
			// The session moves to the integrated commit in the same transaction:
			// it then owns nothing and its next read and write start from the
			// trunk it just made, so the session can integrate again.
			write!(
				input,
				"start\nupdate HEAD {} {}\nupdate {} {} {}\nprepare\n",
				commit,
				expected.head,
				self.branch(session),
				commit,
				expected.overlay
			)
			.map_err(|e| e.to_string())?;
			input.flush().map_err(|e| e.to_string())?;
			for wanted in ["start: ok", "prepare: ok"] {
				let mut line = String::new();
				output.read_line(&mut line).map_err(|e| e.to_string())?;
				if line.trim() != wanted {
					return Err("ref transaction could not prepare; preview again".into());
				}
			}
			if self.branch_name()?.as_deref() != Some(&expected.branch) {
				return Err("reviewed branch changed; preview again".into());
			}
			if cancel.load(Ordering::SeqCst) {
				return Err("cancelled before publication".into());
			}
			dispatched = true; // Even a partial write may have reached the transaction.
			input.write_all(b"commit\n").map_err(|e| e.to_string())?;
			input.flush().map_err(|e| e.to_string())?;
			drop(input);
			let mut line = String::new();
			output.read_line(&mut line).map_err(|e| e.to_string())?;
			if line.trim() != "commit: ok" {
				return Err("ref transaction commit response unavailable".into());
			}
			Ok(())
		})();
		if result.is_err() {
			let _ = transaction.kill();
		}
		let waited = transaction.wait().map_err(|e| e.to_string()).and_then(|s| {
			if s.success() {
				Ok(())
			} else {
				Err("ref transaction failed".into())
			}
		});
		if let Err(error) = result.and(waited) {
			let actual = self
				.git_str(&["rev-parse", &format!("refs/heads/{}", expected.branch)])
				.ok();
			return Err(publication_failure(
				dispatched,
				&commit,
				actual.as_deref().map(str::trim),
				&error,
			));
		}
		Ok(commit)
	}

	/// After an integrate, sync selected owned paths whose index still matches
	/// the parent. Disk bytes already matching the commit, or an already absent
	/// deleted file, need only an index update. Otherwise an unchanged parent
	/// copy receives the exact committed bytes, without checkout filters.
	/// This prevents `git status` showing the integrated change as reverted and
	/// a later plain `git commit` undoing it. Independent disk or staged changes,
	/// and untracked files in the way, are left alone and named in `kept`.
	pub fn sync_integrated(&self, parent: &str, commit: &str, owned: &[Owned]) -> Result<Value> {
		let _repo = self.mutation_lock("repository")?;
		self.sync_integrated_locked(parent, commit, owned)
	}

	/// Retries only the already-proven commit, never a second ref transaction.
	pub(crate) fn retry_integrated_sync(
		&self,
		attempt: &IntegrationAttempt,
		commit: &str,
	) -> Result<Value> {
		let _repo = self.mutation_lock("repository")?;
		if self.branch_name()?.as_deref() != Some(&attempt.review.branch)
			|| self.head()?.as_deref() != Some(commit)
		{
			return Err(
				"original integration branch/HEAD is not checked out; retry refused".into(),
			);
		}
		self.sync_integrated_locked(&attempt.review.head, commit, &attempt.review.owned)
	}

	fn sync_integrated_locked(&self, parent: &str, commit: &str, owned: &[Owned]) -> Result<Value> {
		let mut synced = Vec::new();
		let mut kept = Vec::new();
		for (status, rel) in owned {
			let path = Path::new(rel);
			let spec = literal(path);
			let index_unchanged = self
				.git(&["diff", "--cached", "--quiet", parent, "--", &spec])
				.is_ok();
			let entry = self.entry_at(commit, path)?;
			let materialized = match &entry {
				Some((_, sha)) => {
					std::fs::symlink_metadata(self.root.join(path)).is_ok_and(|m| m.is_file())
						&& std::fs::read(self.root.join(path)).ok()
							== Some(self.git(&["cat-file", "blob", sha])?)
				}
				None if *status == 'D' => std::fs::symlink_metadata(self.root.join(path))
					.is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
				None => false,
			};
			// Idempotent after a lost synchronization ACK: exact index AND disk
			// already at the commit are safe, including completed deletions.
			if materialized
				&& self
					.git(&["diff", "--cached", "--quiet", commit, "--", &spec])
					.is_ok()
			{
				synced.push(rel.clone());
				continue;
			}
			if index_unchanged && materialized {
				let done = match entry {
					Some((mode, sha)) => self.index_update(&[
						"update-index",
						"--add",
						"--cacheinfo",
						&format!("{mode},{sha},{}", printable(path)),
					]),
					None => self.index_update(&[
						"update-index",
						"--force-remove",
						"--",
						&printable(path),
					]),
				};
				if done {
					synced.push(rel.clone());
				} else {
					kept.push(rel.clone());
				}
				continue;
			}
			let untouched = index_unchanged
				&& self.git(&["diff", "--quiet", parent, "--", &spec]).is_ok()
				&& (*status != 'A' || !self.root.join(path).exists());
			let done = untouched && self.sync_path(commit, *status, path);
			if done {
				synced.push(rel.clone());
			} else {
				kept.push(rel.clone());
			}
		}
		Ok(json!({"synced": synced, "kept": kept}))
	}

	/// Put one integrated path on disk and in the shared index. Deletion removes
	/// the index record first and then the file; anything else writes the blob's
	/// exact bytes and stages that blob by sha, never by re-reading the worktree.
	fn sync_path(&self, commit: &str, status: char, path: &Path) -> bool {
		let spec = printable(path);
		if status == 'D' {
			return self.index_update(&["update-index", "--force-remove", "--", &spec])
				&& match std::fs::remove_file(self.root.join(path)) {
					Ok(()) => true,
					Err(e) => e.kind() == std::io::ErrorKind::NotFound,
				};
		}
		let Some((mode, sha)) = self.entry_at(commit, path).ok().flatten() else {
			return false;
		};
		let Ok(content) = self.git(&["cat-file", "blob", &sha]) else {
			return false;
		};
		if self
			.write_disk(&self.root.join(path), &content, &mode)
			.is_err()
		{
			return false;
		}
		self.index_update(&[
			"update-index",
			"--add",
			"--cacheinfo",
			&format!("{mode},{sha},{spec}"),
		])
	}

	/// Update the shared index through plumbing, retrying while another git
	/// process holds `.git/index.lock` for a few milliseconds. Failing leaves
	/// the path in `kept`; it never blocks another agent's git command.
	fn index_update(&self, args: &[&str]) -> bool {
		let pause = INDEX_PATIENCE / INDEX_ATTEMPTS as u32;
		(0..INDEX_ATTEMPTS).any(|attempt| {
			if attempt > 0 {
				std::thread::sleep(pause);
			}
			self.git(args).is_ok()
		})
	}

	/// The (mode, blob) a revision holds for a path.
	fn entry_at(&self, rev: &str, rel: &Path) -> Result<Option<(String, String)>> {
		let listing = self.git_str(&["ls-tree", rev, "--", &literal(rel)])?;
		let mut cols = listing.split_whitespace();
		Ok(match (cols.next(), cols.next(), cols.next()) {
			(Some(mode), Some("blob"), Some(sha)) => Some((mode.to_owned(), sha.to_owned())),
			_ => None,
		})
	}

	/// The merged blob when `base`, `tip` and `head` all hold a text revision
	/// of `rel` and the two changes do not overlap. `None` leaves the path to
	/// the caller's refusal: an added or deleted side has no text to merge.
	fn merge_revision(
		&self,
		base: &str,
		head: &str,
		rel: &str,
		ours: Option<&(String, String)>,
	) -> Result<Option<String>> {
		let Some((_, ours_sha)) = ours else {
			return Ok(None);
		};
		let Some((_, base_sha)) = self.entry_at(base, Path::new(rel))? else {
			return Ok(None);
		};
		let Some((_, theirs_sha)) = self.entry_at(head, Path::new(rel))? else {
			return Ok(None);
		};
		let read = |sha: &str| self.git(&["cat-file", "blob", sha]);
		let merged =
			crate::merge::merge3(&read(&base_sha)?, &read(ours_sha)?, &read(&theirs_sha)?)?;
		match merged {
			Some(bytes) => Ok(Some(self.hash_object(&bytes)?)),
			None => Ok(None),
		}
	}

	/// Whether the branch head commit carries the `Integrated-by: gitfs` trailer.
	pub fn head_is_integrated(&self) -> Result<bool> {
		let Some(head) = self.head()? else {
			return Ok(false);
		};
		let body =
			String::from_utf8_lossy(&self.git(&["log", "-1", "--format=%B", &head])?).into_owned();
		Ok(body.contains("Integrated-by: gitfs"))
	}

	/// Push the current branch; on non-fast-forward, fetch and rebase onto the
	/// remote once, then retry. Returns `Err` only when the push could not
	/// deliver and the commit stays local (the commit itself stays on the
	/// branch either way).
	#[cfg(test)]
	pub fn push(&self, remote: &str, branch: &str) -> Result<String> {
		let head = self.head()?.ok_or("cannot push: no HEAD")?;
		let push = self.git_str(&["push", remote, &format!("{branch}:{branch}")]);
		match push {
			Ok(_) => Ok(head),
			Err(first_err) => {
				// Only a non-fast-forward is retryable; a missing or unreachable
				// remote (e.g. none configured) keeps the commit local with the
				// push's own message.
				if self.git_str(&["fetch", remote, branch]).is_err() {
					return Err(format!(
						"push failed; commit kept locally: {}",
						first_err.trim()
					));
				}
				if self.git_str(&["rebase", "FETCH_HEAD"]).is_ok() {
					let retry = self.git_str(&["push", remote, &format!("{branch}:{branch}")]);
					match retry {
						Ok(_) => {
							let head = self.head()?.unwrap_or(head);
							Ok(head)
						}
						Err(e) => Err(format!("push retry failed; commit kept locally: {e}")),
					}
				} else {
					let _ = self.git_str(&["rebase", "--abort"]);
					Err(format!(
						"push conflict; commit kept locally: {}",
						first_err.trim()
					))
				}
			}
		}
	}

	/// Undo the last integrate: refuse without the trailer, rewind the remote to the
	/// parent under a lease, then mixed-reset the branch so the worktree is
	/// untouched.
	pub fn undo(&self, remote: &str, branch: &str) -> Result<String> {
		if !self.head_is_integrated()? {
			return Err("cannot undo: HEAD is not a gitfs integrate".into());
		}
		let head = self.head()?.ok_or("cannot undo: no HEAD")?;
		let parent = self.git_str(&["rev-parse", "--verify", "--quiet", "HEAD~1"])?;
		let parent = parent.trim().to_owned();
		if parent.is_empty() {
			return Err("cannot undo: nothing to rewind to".into());
		}
		// Remote rewind only when the remote tip is exactly the integrated commit.
		let remote_ref = format!("refs/remotes/{remote}/{branch}");
		let remote_tip = self.git_str(&["rev-parse", "--verify", "--quiet", &remote_ref]);
		if let Ok(tip) = remote_tip {
			let tip = tip.trim().to_owned();
			if !tip.is_empty() && tip == head {
				self.git_str(&[
					"push",
					"--force-with-lease",
					&format!("{branch}:{head}"),
					remote,
					&format!("{parent}:refs/heads/{branch}"),
				])
				.map_err(|e| format!("remote rewind failed: {e}"))?;
			}
		}
		// Mixed reset: branch moves, index and worktree untouched.
		self.git_str(&["reset", "--mixed", &parent])?;
		Ok(parent)
	}

	// ————— materialize helpers —————

	fn deletion_matches(
		&self,
		session: &str,
		rel: &str,
		entry: Option<&LedgerEntry>,
		bytes: &[u8],
	) -> Result<bool> {
		let matches_blob = |blob: &str| {
			self.git(&["cat-file", "blob", blob])
				.map(|prior| prior == bytes)
		};
		if let Some(entry) = entry {
			return match &entry.blob {
				Some(blob) => matches_blob(blob),
				None => Ok(false),
			};
		}
		let path = Path::new(rel);
		if let Some(base) = self.trunk_base(session) {
			if let Some((_, blob)) = self.entry_at(&base, path)? {
				if matches_blob(&blob)? {
					return Ok(true);
				}
			}
		}
		let commit = self.git_str(&[
			"log",
			"-1",
			"--format=%H",
			"--diff-filter=D",
			&self.branch(session),
			"--",
			&literal(path),
		])?;
		if commit.trim().is_empty() {
			return Ok(false);
		}
		match self.entry_at(&format!("{}^", commit.trim()), path)? {
			Some((_, blob)) => matches_blob(&blob),
			None => Ok(false),
		}
	}

	/// Committer timestamp of a commit, seconds since the epoch.
	fn tip_time(&self, tip: &str) -> Result<u64> {
		let s = self.git_str(&["log", "-1", "--format=%ct", tip])?;
		Ok(s.trim().parse().unwrap_or(0))
	}

	/// Committer time of the last session commit touching `rel` (the git-fs
	/// "blob is this old" reference for the materialize guard).
	fn path_time(&self, session: &str, rel: &str) -> Option<u64> {
		self.git_str(&[
			"log",
			"-1",
			"--format=%ct",
			&self.branch(session),
			"--",
			rel,
		])
		.ok()
		.and_then(|s| s.trim().parse().ok())
	}

	fn write_disk(&self, disk: &Path, content: &[u8], mode: &str) -> Result<Option<String>> {
		if let Some(parent) = disk.parent() {
			std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
		}
		write_file_retrying(disk, content).map_err(|e| {
			format!(
				"write failed after {} attempts: {e}; on Windows a running executable holds its file against replacement, stop it or rename it, then materialize again",
				LOCK_ATTEMPTS
			)
		})?;
		if mode == "100755" {
			// Executable bit is POSIX-only; Windows has no equivalent to set.
			#[cfg(unix)]
			if let Err(e) =
				std::fs::set_permissions(disk, std::os::unix::fs::PermissionsExt::from_mode(0o755))
			{
				return Ok(Some(format!("permissions failed: {e}")));
			}
		}
		Ok(None)
	}

	/// Record that disk and branch agree on `rel` right now, as a materialize
	/// would: the post-edit hook calls it after mirroring a native edit, so the
	/// next materialize does not read the disk copy as a foreign change.
	pub fn note_materialized(&self, session: &str, rel: &str) -> Result<()> {
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		let sha = self
			.selected_blob(session, Path::new(rel))?
			.map(|(sha, _)| sha);
		let expected = sha
			.as_ref()
			.map(|sha| self.git(&["cat-file", "blob", sha]))
			.transpose()?;
		let actual = match std::fs::read(self.root.join(rel)) {
			Ok(bytes) => Some(bytes),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
			Err(e) => return Err(format!("{rel}: cannot verify captured disk content: {e}")),
		};
		let changed = actual != expected;
		let mut ledger = self.load_ledger(session)?;
		ledger.insert(
			rel.to_owned(),
			LedgerEntry {
				blob: sha,
				mtime: fs_mtime(&self.root.join(rel)).unwrap_or_else(now_ms),
			},
		);
		self.save_ledger(session, &ledger)?;
		if changed {
			return Err(format!("partial_success: {rel} already captured but disk changed during capture; do not automatically retry"));
		}
		Ok(())
	}

	fn load_ledger(&self, session: &str) -> Result<HashMap<String, LedgerEntry>> {
		self.load_json(&format!("{session}.materialized.json"))
	}

	fn save_ledger(&self, session: &str, ledger: &HashMap<String, LedgerEntry>) -> Result<()> {
		self.save_json(&format!("{session}.materialized.json"), ledger)
	}

	/// One of this store's own JSON side files, absent reading as empty.
	fn load_json<T: serde::de::DeserializeOwned + Default>(&self, name: &str) -> Result<T> {
		let Ok(text) = std::fs::read_to_string(self.dir.join(name)) else {
			return Ok(T::default());
		};
		serde_json::from_str(&text).map_err(|e| e.to_string())
	}

	fn save_json<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
		std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
		let path = self.dir.join(name);
		let tmp = path.with_extension("tmp");
		let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
		std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
		std::fs::rename(&tmp, path).map_err(|e| e.to_string())
	}
}

// Integration intent survives process restarts and is never garbage-collected while
// the ref transaction could be in flight. Git remains the authority for commits.
impl Store {
	pub(crate) fn integration_attempt(&self, revision: &str) -> Result<Option<IntegrationAttempt>> {
		if revision.len() != 64 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
			return Ok(None);
		}
		let file = self.dir.join(format!("integration-{revision}.json"));
		let metadata = match std::fs::symlink_metadata(&file) {
			Ok(metadata) => metadata,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
			Err(error) => return Err(format!("cannot stat integration recovery: {error}")),
		};
		if !metadata.file_type().is_file() || metadata.len() > 4 * 1024 * 1024 {
			return Err("unsafe or oversized integration recovery record".into());
		}
		let text = match std::fs::read_to_string(file) {
			Ok(text) => text,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
			Err(error) => return Err(format!("cannot read integration recovery: {error}")),
		};
		let attempt: IntegrationAttempt = serde_json::from_str(&text)
			.map_err(|error| format!("invalid integration recovery: {error}"))?;
		if attempt.schema != "gitfs-integration-attempt/v1"
			|| attempt.revision != revision
			|| attempt.session.is_empty()
			|| attempt.review.repository != self.repository_identity()?
			|| attempt.review.branch.is_empty()
			|| attempt.review.overlay.is_empty()
		{
			return Err("integration recovery binding mismatch; refusing publication".into());
		}
		Ok(Some(attempt))
	}

	pub(crate) fn save_integration_attempt(&self, attempt: &IntegrationAttempt) -> Result<()> {
		if attempt.revision.len() != 64
			|| !attempt
				.revision
				.bytes()
				.all(|byte| byte.is_ascii_hexdigit())
			|| attempt.schema != "gitfs-integration-attempt/v1"
			|| attempt.review.repository != self.repository_identity()?
		{
			return Err("invalid integration recovery binding".into());
		}
		let _lock = self.mutation_lock("repository")?;
		if let Some(prior) = self.integration_attempt(&attempt.revision)? {
			if prior.schema != attempt.schema
				|| prior.revision != attempt.revision
				|| prior.session != attempt.session
				|| prior.review != attempt.review
				|| prior.subject != attempt.subject
				|| prior.author_name != attempt.author_name
				|| prior.author_email != attempt.author_email
				|| prior.committed.is_some() && prior.committed != attempt.committed
				|| prior.worktree.is_some() && attempt.worktree.is_none()
			{
				return Err("integration recovery is already bound to another outcome".into());
			}
		} else if attempt.committed.is_some() || attempt.worktree.is_some() {
			return Err("cannot record integration result without durable intent".into());
		}
		std::fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
		let bytes = serde_json::to_vec_pretty(attempt).map_err(|error| error.to_string())?;
		if bytes.len() > 4 * 1024 * 1024 {
			return Err("integration recovery exceeds size limit".into());
		}
		crate::hook::atomic_write(
			&self
				.dir
				.join(format!("integration-{}.json", attempt.revision)),
			&bytes,
		)
	}
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct LedgerEntry {
	blob: Option<String>,
	mtime: u64,
}

/// The last millisecond of a whole-second commit time.
fn seconds_end_ms(seconds: u64) -> u64 {
	seconds
		.saturating_add(1)
		.saturating_mul(1000)
		.saturating_sub(1)
}

fn now_ms() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_millis() as u64)
		.unwrap_or(0)
}

/// Attempts at a worktree write while a Windows process holds the file open,
/// and the pause between them (~1 s in all).
const LOCK_ATTEMPTS: usize = 20;
const LOCK_PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// Write `content` to `path`, retrying the sharing and lock violations a
/// running executable (a build artifact, a test binary) raises while it holds
/// the file open. Every other error returns at once.
fn write_file_retrying(path: &Path, content: &[u8]) -> std::io::Result<()> {
	let mut attempt = 0;
	loop {
		match std::fs::write(path, content) {
			Err(e) if transient_lock(&e) && attempt + 1 < LOCK_ATTEMPTS => {
				attempt += 1;
				std::thread::sleep(LOCK_PAUSE);
			}
			other => return other,
		}
	}
}

#[cfg(windows)]
fn transient_lock(error: &std::io::Error) -> bool {
	// ERROR_SHARING_VIOLATION (32) and ERROR_LOCK_VIOLATION (33).
	matches!(error.raw_os_error(), Some(32 | 33))
}
#[cfg(not(windows))]
fn transient_lock(_: &std::io::Error) -> bool {
	false
}

fn fs_mtime(path: &Path) -> Option<u64> {
	std::fs::metadata(path)
		.and_then(|m| m.modified())
		.ok()
		.and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
		.map(|d| d.as_millis() as u64)
}

/// A pathspec that matches exactly this path, with no glob or magic.
fn literal(rel: &Path) -> String {
	format!(":(literal){}", printable(rel))
}

/// The (device, file) number pair naming the filesystem object behind `path`, so the reviewed
/// repository identity changes when a path is replaced by a different directory.
#[cfg(unix)]
pub(crate) fn file_id(path: &Path) -> Result<(u64, u64)> {
	use std::os::unix::fs::MetadataExt;
	let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
	Ok((meta.dev(), meta.ino()))
}
#[cfg(windows)]
pub(crate) fn file_id(path: &Path) -> Result<(u64, u64)> {
	use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
	use windows_sys::Win32::Storage::FileSystem::{
		GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
	};
	// A directory opens only with backup semantics; access 0 reads its attributes without
	// holding any data access.
	let file = std::fs::OpenOptions::new()
		.access_mode(0)
		.custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
		.open(path)
		.map_err(|e| format!("file identity unavailable for {}: {e}", path.display()))?;
	let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
	if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
		return Err(format!(
			"file identity unavailable for {}: {}",
			path.display(),
			std::io::Error::last_os_error()
		));
	}
	Ok((
		u64::from(info.dwVolumeSerialNumber),
		(u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
	))
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn file_id(_: &Path) -> Result<(u64, u64)> {
	Err("reviewed repository identity requires file ids (macOS, Linux or Windows)".into())
}

fn var_str(s: String) -> Result<String> {
	trimmed_opt(s).ok_or_else(|| "empty git output".into())
}

fn parse_name_status(out: &[u8]) -> Vec<Owned> {
	// -z format: status NUL path NUL [...]
	let text = String::from_utf8_lossy(out);
	let parts: Vec<&str> = text.split('\0').collect();
	let mut rows = vec![];
	let mut i = 0;
	while i + 1 < parts.len() {
		let status = parts[i];
		let path = parts[i + 1];
		if let Some(ch) = status.chars().next() {
			if !path.is_empty() {
				rows.push((ch, path.to_owned()));
			}
		}
		i += 2;
	}
	rows
}

#[cfg(test)]
#[path = "../tests/unit/store/tests.rs"]
mod tests;

/// The repository's git directory, absolute, for a checkout root.
pub fn git_dir(root: &Path) -> Result<PathBuf> {
	let out = run(root, None, &["rev-parse", "--absolute-git-dir"], None, None)?;
	let dir: PathBuf = Path::new(String::from_utf8_lossy(&out).trim())
		.components()
		.collect();
	if dir.as_os_str().is_empty() {
		return Err("git directory unknown".into());
	}
	Ok(dir)
}

/// Keep the store directory out of the repository without touching its
/// `.gitignore`: `.git/info/exclude` is private to this checkout. Best
/// effort; a store that cannot be excluded still works.
fn exclude_private(root: &Path, store_dir: &str) {
	let Ok(dir) = git_dir(root) else { return };
	let exclude = dir.join("info/exclude");
	let line = format!("/{}/", store_dir.trim_matches('/'));
	let current = std::fs::read_to_string(&exclude).unwrap_or_default();
	if current.lines().any(|l| l.trim() == line) {
		return;
	}
	let _ = std::fs::create_dir_all(dir.join("info"));
	let mut text = current;
	if !text.is_empty() && !text.ends_with('\n') {
		text.push('\n');
	}
	text.push_str(&line);
	text.push('\n');
	let _ = std::fs::write(&exclude, text);
}
