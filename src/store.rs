//! Session overlay store: one branch `refs/gitfs/<session>` per agent session,
//! written through git plumbing so the working tree and the real index are
//! never touched. Integrate overlays the session's owned paths onto the current
//! branch head, so trunk movement between session start and integrate can't show
//! stale full trees.

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

/// How long integrate waits for `.git/index.lock` held by someone else. Git's own
/// readers (`git status` from an editor, a prompt or the buffer git provider)
/// take the lock for a few milliseconds to refresh stat data; a single
/// `create_new` attempt refused a integrate whenever one of them was mid-refresh.
const INDEX_PATIENCE: std::time::Duration = std::time::Duration::from_secs(2);

struct IndexLock(PathBuf);
impl IndexLock {
	/// Create the lock file, retrying while another process holds it, for at
	/// most `patience`. A lock held longer is a real writer: refuse.
	fn take(lock: PathBuf, patience: std::time::Duration) -> Result<IndexLock> {
		let start = std::time::Instant::now();
		loop {
			match std::fs::OpenOptions::new()
				.write(true)
				.create_new(true)
				.open(&lock)
			{
				Ok(_) => return Ok(IndexLock(lock)),
				Err(e)
					if e.kind() == std::io::ErrorKind::AlreadyExists
						&& start.elapsed() < patience =>
				{
					std::thread::sleep(std::time::Duration::from_millis(10));
				}
				Err(e) => return Err(format!("shared index busy; preview again: {e}")),
			}
		}
	}
}
impl Drop for IndexLock {
	fn drop(&mut self) {
		let _ = std::fs::remove_file(&self.0);
	}
}

/// File locks coordinate threads and kern processes. Session indexes are
/// independent; only working-tree and trunk changes share the repository lock.
struct MutationLock(std::fs::File);
impl Drop for MutationLock {
	fn drop(&mut self) {
		let _ = self.0.unlock();
	}
}

const IDENT_NAME: &str = "kern gitfs";
const IDENT_EMAIL: &str = "gitfs@kern.local";

#[derive(Clone)]
pub struct Store {
	root: PathBuf,
	dir: PathBuf,
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
	/// directory inside it) and the runtime store dir under it.
	pub fn at(&self, cwd: &Path) -> Result<Store> {
		let store = self.at_readonly(cwd)?;
		std::fs::create_dir_all(&store.dir).map_err(|e| format!("cannot create store dir: {e}"))?;
		exclude_private(&store.root, &self.store_dir);
		Ok(store)
	}

	pub fn at_readonly(&self, cwd: &Path) -> Result<Store> {
		let s = run(cwd, &["rev-parse", "--show-toplevel"], None, None)
			.map_err(|_| "not inside a git repository".to_string())?;
		let root = PathBuf::from(String::from_utf8_lossy(&s).trim());
		if root.as_os_str().is_empty() {
			return Err("not inside a git repository".into());
		}
		let dir = root.join(&self.store_dir);
		Ok(Store { root, dir })
	}
}

pub struct Materialize {
	pub applied: Vec<String>,
	pub conflicts: Vec<(String, String)>,
	pub errors: Vec<String>,
}

impl Materialize {
	pub fn to_value(&self) -> Value {
		json!({
			"applied": self.applied,
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
fn git_cmd(root: &Path, args: &[&str], index: Option<&Path>) -> Command {
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
		cmd.env("GIT_INDEX_FILE", index);
	}
	cmd
}

/// Run git against `root` with a fixed identity, hooks disabled, and an
/// optional private index file (never the working tree's index).
fn run(root: &Path, args: &[&str], index: Option<&Path>, stdin: Option<&[u8]>) -> Result<Vec<u8>> {
	let mut cmd = git_cmd(root, args, index);
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
fn run_tolerant(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
	let mut cmd = git_cmd(root, args, None);
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
		run(&self.root, args, None, None)
	}

	pub fn git_str(&self, args: &[&str]) -> Result<String> {
		String::from_utf8(self.git(args)?).map_err(|e| e.to_string())
	}

	/// The session branch tip, or None when the session has written nothing.
	pub fn tip(&self, session: &str) -> Result<Option<String>> {
		let out = run_tolerant(
			&self.root,
			&["rev-parse", "--verify", "--quiet", &self.branch(session)],
		)?;
		Ok(trimmed_opt(lossy(out)))
	}

	pub fn head(&self) -> Result<Option<String>> {
		let out = run_tolerant(&self.root, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
		Ok(trimmed_opt(lossy(out)))
	}

	/// Current branch symbolic name, or None on a detached HEAD.
	pub fn branch_name(&self) -> Result<Option<String>> {
		let out = run_tolerant(&self.root, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
		Ok(trimmed_opt(lossy(out)))
	}

	/// Blob sha and mode for a session path, if the session wrote it.
	pub fn blob_for(&self, session: &str, rel: &Path) -> Result<Option<(String, String)>> {
		// A session that has written nothing has no branch ref for ls-tree.
		if self.tip(session)?.is_none() {
			return Ok(None);
		}
		let out = self.git(&["ls-tree", &self.branch(session), "--", &printable(rel)])?;
		let line = std::str::from_utf8(&out).map_err(|e| e.to_string())?;
		let mut cols = line.split_whitespace();
		let mode = cols.next().unwrap_or("100644").to_owned();
		let _kind = cols.next();
		match cols.next().map(str::to_owned) {
			Some(sha) => Ok(Some((sha, mode))),
			None => Ok(None),
		}
	}

	/// Git mode a session write of `rel` keeps: the session's own blob mode, else
	/// the file on disk (a tracked file before the session's first write, or an
	/// untracked one), else a plain file.
	pub fn mode_for(&self, session: &str, rel: &Path) -> Result<String> {
		if let Some((_, mode)) = self.blob_for(session, rel)? {
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

	/// The merge base of the session branch and trunk, if the session wrote
	/// anything. `None` when there is nothing to base a diff against.
	pub fn merge_base(&self, session: &str) -> Result<Option<String>> {
		match self.tip(session)? {
			None => Ok(None),
			Some(_) => match self.git_str(&["merge-base", "HEAD", &self.branch(session)]) {
				Ok(s) => Ok(trimmed_opt(s)),
				Err(_) => Ok(None),
			},
		}
	}

	/// Paths the session owns and how they changed relative to trunk at
	/// session start: `(status, path)` with status A/M/D.
	pub fn owned_paths(&self, session: &str) -> Result<Vec<Owned>> {
		let Some(base) = self.merge_base(session)? else {
			return Ok(vec![]);
		};
		let branch = self.branch(session);
		let range = format!("{base}...{branch}");
		let out = self.git(&["diff", "--name-status", "--no-renames", "-z", &range])?;
		Ok(parse_name_status(&out))
	}

	/// How the session changed one path against trunk at session start: `None`
	/// when it did not, `Some(None)` when it deleted it, else its content.
	pub fn session_change(&self, session: &str, rel: &Path) -> Result<Option<Option<Vec<u8>>>> {
		let Some(base) = self.merge_base(session)? else {
			return Ok(None);
		};
		let range = format!("{base}...{}", self.branch(session));
		let out = self.git(&[
			"diff",
			"--name-status",
			"--no-renames",
			"-z",
			&range,
			"--",
			&literal(rel),
		])?;
		match parse_name_status(&out).first() {
			None => Ok(None),
			Some(('D', _)) => Ok(Some(None)),
			Some(_) => Ok(Some(self.read(session, rel)?)),
		}
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
			&["merge-base", "--is-ancestor", ancestor, of],
			None,
		)
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
		let parent = format!("{commit}^");
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
		if self.session_change(session, path)?.is_none() {
			return Err(format!("rollback: session {session} does not change {rel}"));
		}
		let base = self
			.merge_base(session)?
			.ok_or("session has no branch to roll back")?;
		let listing = self.git_str(&["ls-tree", &base, "--", &literal(path)])?;
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
	/// earlier integrate) is dropped; a path trunk moved to anything else refuses
	/// the whole catchup, naming every such path and the way out, with nothing
	/// written. When nothing is left to replay the session ref is deleted and
	/// the session starts empty at trunk.
	pub(crate) fn catchup_captured(&self, session: &str, capture: &mut Capture) -> Result<Value> {
		capture.check_session(session)?;
		let _repo = self.mutation_lock("repository")?;
		let _session = self.mutation_lock(&format!("session:{session}"))?;
		let head = self.head()?.ok_or("catchup: trunk has no commit yet")?;
		let at_trunk = |base: Option<&str>| json!({"caught_up":true,"base":base,"head":head,"replayed":[],"dropped":[],"ref_deleted":false});
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
		let entry = |rev: &str, rel: &str| -> Result<Option<(String, String)>> {
			let listing = self.git_str(&["ls-tree", rev, "--", &literal(Path::new(rel))])?;
			let mut cols = listing.split_whitespace();
			Ok(match (cols.next(), cols.next(), cols.next()) {
				(Some(mode), Some("blob"), Some(sha)) => Some((mode.to_owned(), sha.to_owned())),
				_ => None,
			})
		};
		let (mut replay, mut dropped, mut conflicts) = (Vec::new(), Vec::new(), Vec::new());
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
			if moved.is_empty() {
				replay.push((rel, mine));
			} else if mine == entry(&head, &rel)? {
				dropped.push(rel);
			} else {
				conflicts.push(rel);
			}
		}
		if !conflicts.is_empty() {
			return Err(format!(
				"catchup: trunk changed {} since this session's base {base} and the session holds other content; for each: tool.gitfs.rollback {{path}}, tool.read it again, rewrite on top of trunk, then catchup",
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
				json!({"caught_up":true,"base":base,"head":head,"replayed":[],"dropped":dropped,"ref_deleted":true}),
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
			json!({"caught_up":true,"base":base,"head":head,"tip":parent,"replayed":replayed,"dropped":dropped,"ref_deleted":false,"worktree":"materialize again to copy the caught-up paths in"}),
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
			run(&self.root, &["read-tree", "--empty"], Some(&index), None)?;
		} else {
			run(&self.root, &["read-tree", base_tree], Some(&index), None)?;
		}
		for (mode, blob, rel) in adds {
			run(
				&self.root,
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
				&["update-index", "--force-remove", "--", &printable(rel)],
				Some(&index),
				None,
			)?;
		}
		let out = run(&self.root, &["write-tree"], Some(&index), None)?;
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
		let claimed = printable(rel);
		let fresh_claim = !self
			.owned_paths(session)?
			.iter()
			.any(|(_, owned)| owned == &claimed);
		let mut evidence = capture.reserve(self, rel, op, Storage::GitfsOverlay)?;
		if let Some(e) = &mut evidence {
			e.overlay_commit.before = prev.clone();
			if !base_tree.is_empty() {
				let output = self.git(&["ls-tree", &base_tree, "--", &printable(rel)])?;
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
		let commit = self.commit_tree(
			&tree,
			parent.as_deref(),
			&format!("gitfs: {subject}\n\nGitfs-Session: {session}"),
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
		let mut ledger = self.load_ledger(session).unwrap_or_default();
		let mut result = Materialize {
			applied: vec![],
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
		for (status, rel) in self.owned_paths(session)? {
			if let Some(allowed) = paths {
				if !allowed.iter().any(|p| p == &rel) {
					continue;
				}
			}
			let disk = self.root.join(&rel);
			let disk_mtime = fs_mtime(&disk);
			match status {
				'M' | 'A' => {
					let (sha, mode) = match self.blob_for(session, Path::new(&rel)) {
						Ok(Some(v)) => v,
						Ok(None) => continue,
						Err(e) => {
							result.errors.push(format!("{rel}: {e}"));
							continue;
						}
					};
					let content = match self.read(session, Path::new(&rel)) {
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
								blob: sha,
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
							if disk_mtime.is_some() && disk_mtime != Some(e.mtime) {
								Some("file modified on disk since it was materialized".to_string())
							} else {
								None
							}
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
						result.conflicts.push((rel.clone(), reason));
						continue;
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
							blob: sha,
							mtime: fs_mtime(&disk).unwrap_or_else(now_ms),
						},
					);
					result.applied.push(rel.clone());
				}
				'D' => {
					let entry = ledger.get(&rel).cloned();
					let conflict = match &entry {
						Some(e) if !force => {
							if disk_mtime.is_some() && disk_mtime != Some(e.mtime) {
								Some("file modified on disk since it was materialized".to_string())
							} else {
								None
							}
						}
						None => {
							if let Some(dm) = disk_mtime {
								if dm > tip_ts && !force {
									Some(
										"on-disk file is newer than the gitfs deletion".to_string(),
									)
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
						result.conflicts.push((rel.clone(), reason));
						continue;
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

					ledger.remove(&rel);
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
		if revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Ok(None);
		}
		let out = run_tolerant(
			&self.root,
			&[
				"log",
				"-n",
				"1",
				"--fixed-strings",
				&format!("--grep=Gitfs-Review: {revision}"),
				"--format=%H%x00%T%x00%P%x00%s%x00%b",
				"HEAD",
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
				"read revision behind trunk in {}: trunk moved after this session read them, so the write is not a merge to make; tool.gitfs.rollback your commits on each path, tool.read it again, write on top of what trunk now holds and ship again",
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

	#[cfg(unix)]
	pub(crate) fn repository_identity(&self) -> Result<String> {
		use std::os::unix::fs::MetadataExt;
		let root = self.root.canonicalize().map_err(|e| e.to_string())?;
		let git_dir = PathBuf::from(self.git_str(&["rev-parse", "--absolute-git-dir"])?.trim())
			.canonicalize()
			.map_err(|e| e.to_string())?;
		let root_meta = std::fs::metadata(&root).map_err(|e| e.to_string())?;
		let git_meta = std::fs::metadata(&git_dir).map_err(|e| e.to_string())?;
		Ok(format!(
			"{:x}",
			Sha256::digest(
				serde_json::to_vec(&json!([
					root,
					git_dir,
					root_meta.dev(),
					root_meta.ino(),
					git_meta.dev(),
					git_meta.ino()
				]))
				.unwrap()
			)
		))
	}
	#[cfg(not(unix))]
	pub(crate) fn repository_identity(&self) -> Result<String> {
		Err("reviewed repository identity requires a supported macOS/Linux filesystem".into())
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
		let index = self.shared_index()?;
		let _index = IndexLock::take(
			PathBuf::from(format!("{}.lock", index.display())),
			INDEX_PATIENCE,
		)?;
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
			&["commit-tree", &expected.tree, "-p", &expected.head],
			None,
		)
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
		let mut transaction = git_cmd(&self.root, &["update-ref", "--stdin"], None)
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

	/// After a integrate, bring the shared index and working tree up to the commit
	/// on every owned path they still hold as the parent held it, so `git
	/// status` stops showing the integrated change as reverted and a later plain
	/// `git commit -a` cannot undo it. A path somebody changed on disk or in
	/// the index since the parent — or an untracked file in the way — is left
	/// alone and named in `kept`.
	pub fn sync_integrated(&self, parent: &str, commit: &str, owned: &[Owned]) -> Result<Value> {
		let _repo = self.mutation_lock("repository")?;
		let mut synced = Vec::new();
		let mut kept = Vec::new();
		for (status, rel) in owned {
			let path = Path::new(rel);
			let spec = literal(path);
			let untouched = self.git(&["diff", "--quiet", parent, "--", &spec]).is_ok()
				&& self
					.git(&["diff", "--cached", "--quiet", parent, "--", &spec])
					.is_ok() && (*status != 'A' || !self.root.join(path).exists());
			// The shared index is locked for a few ms at a time by other
			// readers (`git status` pollers): retry briefly, or the integrated
			// path stays in the index as the parent had it, a staged revert.
			// ponytail: fixed 2 s of retries; a real long-held lock still keeps the path.
			let done = untouched
				&& (0..20).any(|attempt| {
					if attempt > 0 {
						std::thread::sleep(std::time::Duration::from_millis(100));
					}
					if *status == 'D' {
						self.git(&["rm", "-q", "--cached", "--", &spec]).is_ok()
					} else {
						self.git(&[
							"restore",
							"--source",
							commit,
							"--staged",
							"--worktree",
							"--",
							&spec,
						])
						.is_ok()
					}
				}) && (*status != 'D' || std::fs::remove_file(self.root.join(path)).is_ok());
			if done {
				synced.push(rel.clone());
			} else {
				kept.push(rel.clone());
			}
		}
		Ok(json!({"synced": synced, "kept": kept}))
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
		std::fs::write(disk, content).map_err(|e| e.to_string())?;
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
		let Some((sha, _)) = self.blob_for(session, Path::new(rel))? else {
			return Ok(());
		};
		let mut ledger = self.load_ledger(session).unwrap_or_default();
		ledger.insert(
			rel.to_owned(),
			LedgerEntry {
				blob: sha,
				mtime: fs_mtime(&self.root.join(rel)).unwrap_or_else(now_ms),
			},
		);
		self.save_ledger(session, &ledger)
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct LedgerEntry {
	blob: String,
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
	let out = run(root, &["rev-parse", "--absolute-git-dir"], None, None)?;
	let dir = PathBuf::from(String::from_utf8_lossy(&out).trim());
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
