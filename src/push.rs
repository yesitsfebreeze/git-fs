//! Durable, non-replayed ordinary Git pushes. Raw transport data never enters receipts.
use crate::{store::StoreConfig, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::Arc;
use std::time::{Duration, Instant};

// Caps, not constants: declared in `kern.md` under `limits`.
fn capacity() -> usize {
	crate::limits::limits().push_records
}
fn record_limit() -> usize {
	crate::limits::limits().push_record_bytes
}
#[cfg_attr(not(unix), allow(dead_code))]
fn output_limit() -> usize {
	crate::limits::limits().push_output_bytes
}
fn digest(bytes: &[u8]) -> String {
	format!("{:x}", Sha256::digest(bytes))
}
fn oid(s: &str) -> bool {
	[40, 64].contains(&s.len())
		&& s.bytes()
			.all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
		&& s.bytes().any(|c| c != b'0')
}
fn name(s: &str, limit: usize) -> bool {
	!s.is_empty()
		&& s.len() <= limit
		&& !s.starts_with('-')
		&& s.bytes()
			.all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

fn session_name(s: &str) -> bool {
	!s.is_empty()
		&& s.len() <= 128
		&& s.bytes()
			.all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Binding {
	repository: String,
	session: String,
	remote: String,
	reference: String,
	commit: String,
	expected_remote_head: Option<String>,
	url_digest: String,
	timeout_ms: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Status {
	Unknown,
	Refused,
	Confirmed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
	schema: String,
	operation_id: String,
	revision: String,
	binding: Binding,
	status: Status,
	reason: String,
}
impl Record {
	fn value(&self) -> Value {
		serde_json::to_value(self).expect("serializable receipt")
	}
	fn valid(&self) -> bool {
		self.schema == "gitfs-push/v1"
			&& name(&self.operation_id, 64)
			&& session_name(&self.binding.session)
			&& self.revision == revision(&self.binding)
			&& valid_binding(&self.binding)
			&& [
				"dispatch_pending_or_unknown",
				"cancelled_before_dispatch",
				"hook_refused",
				"exact_remote_confirmed",
				"remote_not_confirmed",
				"binding_unavailable",
			]
			.contains(&self.reason.as_str())
	}
}
fn valid_binding(b: &Binding) -> bool {
	b.repository.len() == 64
		&& b.repository.bytes().all(|c| c.is_ascii_hexdigit())
		&& name(&b.remote, 64)
		&& b.reference.starts_with("refs/heads/")
		&& b.reference.len() <= 256
		&& b.reference
			.bytes()
			.all(|c| c.is_ascii_alphanumeric() || b"/_.-".contains(&c))
		&& oid(&b.commit)
		&& b.expected_remote_head.as_deref().is_none_or(oid)
		&& b.url_digest.len() == 64
		&& b.url_digest.bytes().all(|c| c.is_ascii_hexdigit())
		&& (100..=60000).contains(&b.timeout_ms)
}
fn revision(binding: &Binding) -> String {
	digest(&serde_json::to_vec(binding).expect("serializable binding"))
}
fn record_key(session: &str, id: &str) -> String {
	digest(&serde_json::to_vec(&(session, id)).expect("serializable key"))
}

struct Context<'a> {
	root: PathBuf,
	dir: PathBuf,
	deadline: Instant,
	cancel: &'a AtomicBool,
}
struct Output {
	success: bool,
	bytes: Vec<u8>,
}
impl Context<'_> {
	fn check(&self) -> Result<()> {
		if self.cancel.load(Ordering::SeqCst) {
			return Err("push cancelled".into());
		}
		if Instant::now() >= self.deadline {
			return Err("push deadline exceeded".into());
		}
		Ok(())
	}
	fn command(&self, args: &[&str]) -> Command {
		let mut c = Command::new("git");
		c.arg("-C")
			.arg(&self.root)
			.args([
				"-c",
				"core.hooksPath=/dev/null",
				"-c",
				"push.followTags=false",
				"-c",
				"push.recurseSubmodules=no",
			])
			.args(args)
			.env("GIT_TERMINAL_PROMPT", "0")
			.env("GIT_NO_REPLACE_OBJECTS", "1");
		for key in [
			"GIT_DIR",
			"GIT_WORK_TREE",
			"GIT_COMMON_DIR",
			"GIT_INDEX_FILE",
			"GIT_OBJECT_DIRECTORY",
			"GIT_ALTERNATE_OBJECT_DIRECTORIES",
		] {
			c.env_remove(key);
		}
		c
	}
	fn run(&self, mut command: Command) -> Result<Output> {
		self.check()?;
		#[cfg(not(unix))]
		{
			let _ = &mut command;
			return Err("bounded push requires macOS/Linux".into());
		}
		#[cfg(unix)]
		{
			use std::os::unix::process::CommandExt;
			command
				.process_group(0)
				.stdin(Stdio::null())
				.stdout(Stdio::piped())
				.stderr(Stdio::piped());
			let mut child = command.spawn().map_err(|_| "Git process unavailable")?;
			let group = child.id();
			let overflow = Arc::new(AtomicBool::new(false));
			let read = |mut pipe: Box<dyn Read + Send>, overflow: Arc<AtomicBool>| {
				std::thread::spawn(move || {
					let mut bytes = Vec::new();
					let mut chunk = [0; 4096];
					loop {
						match pipe.read(&mut chunk) {
							Ok(0) => break,
							Ok(n) => {
								if bytes.len() + n > output_limit() {
									overflow.store(true, Ordering::SeqCst);
									break;
								}
								bytes.extend_from_slice(&chunk[..n]);
							}
							Err(_) => {
								overflow.store(true, Ordering::SeqCst);
								break;
							}
						}
					}
					bytes
				})
			};
			let out = read(
				Box::new(child.stdout.take().ok_or("missing Git output")?),
				overflow.clone(),
			);
			let err = read(
				Box::new(child.stderr.take().ok_or("missing Git error output")?),
				overflow.clone(),
			);
			let result = loop {
				if let Err(e) = self.check() {
					break Err(e);
				}
				if overflow.load(Ordering::SeqCst) {
					break Err("Git output exceeds limit".into());
				}
				match child.try_wait() {
					Ok(Some(s)) => break Ok(s.success()),
					Ok(None) => std::thread::sleep(Duration::from_millis(2)),
					Err(_) => break Err("Git process outcome unavailable".into()),
				}
			};
			// Kill the entire group even after the parent exits: helpers may own its pipes.
			// A signal, not a `/bin/kill` child: a sandbox that forbids exec
			// would leave the group alive and `wait` blocked on it.
			unsafe {
				libc::killpg(group as libc::pid_t, libc::SIGKILL);
			}
			let _ = child.wait();
			// A descendant that left the group can still hold the pipes open;
			// the readers get the same deadline, never an unbounded join.
			while !(out.is_finished() && err.is_finished()) {
				if let Err(e) = self.check() {
					return Err(result.err().unwrap_or(e));
				}
				std::thread::sleep(Duration::from_millis(2));
			}
			let bytes = out.join().map_err(|_| "Git output reader failed")?;
			let _ = err.join().map_err(|_| "Git error reader failed")?;
			let success = result?;
			if overflow.load(Ordering::SeqCst) {
				return Err("Git output exceeds limit".into());
			}
			self.check()?;
			Ok(Output { success, bytes })
		}
	}
	fn git(&self, args: &[&str]) -> Result<String> {
		let o = self.run(self.command(args))?;
		if !o.success {
			return Err("Git input or transport unavailable".into());
		}
		String::from_utf8(o.bytes).map_err(|_| "Git output is not UTF-8".into())
	}
	fn repository(&self) -> Result<String> {
		#[cfg(not(unix))]
		{
			Err("push repository identity unsupported".into())
		}
		#[cfg(unix)]
		{
			use std::os::unix::fs::MetadataExt;
			let root = self
				.root
				.canonicalize()
				.map_err(|_| "repository unavailable")?;
			let dir = git_path(&self.git(&["rev-parse", "--absolute-git-dir"])?)?
				.canonicalize()
				.map_err(|_| "repository unavailable")?;
			let a = fs::metadata(&root).map_err(|_| "repository unavailable")?;
			let b = fs::metadata(&dir).map_err(|_| "repository unavailable")?;
			Ok(digest(
				&serde_json::to_vec(&(root, dir, a.dev(), a.ino(), b.dev(), b.ino()))
					.expect("identity"),
			))
		}
	}
	fn url(&self, remote: &str) -> Result<String> {
		let rewrites = self.run(self.command(&[
			"config",
			"--get-regexp",
			r"^url\..*\.(insteadof|pushinsteadof)$",
		]))?;
		if !rewrites.bytes.is_empty() {
			return Err("URL rewrite rules are unsupported for recorded push".into());
		}
		let mirror =
			self.run(self.command(&["config", "--get", &format!("remote.{remote}.mirror")]))?;
		if mirror.success && String::from_utf8_lossy(&mirror.bytes).trim() != "false" {
			return Err("mirrored remote unsupported".into());
		}
		let value = self.git(&["remote", "get-url", "--push", "--all", remote])?;
		let urls: Vec<_> = value.lines().collect();
		if urls.len() != 1
			|| urls[0].is_empty()
			|| urls[0].len() > 4096
			|| urls[0].chars().any(char::is_control)
			|| urls[0].starts_with('-')
		{
			return Err("one bounded push URL required".into());
		}
		Ok(urls[0].into())
	}
	fn remote_head(&self, url: &str, reference: &str) -> Result<Option<String>> {
		let data = self.git(&["ls-remote", "--refs", "--", url, reference])?;
		if data.is_empty() {
			return Ok(None);
		}
		let lines: Vec<_> = data.lines().collect();
		if lines.len() != 1 {
			return Err("remote ref response ambiguous".into());
		}
		let pair: Vec<_> = lines[0].split('\t').collect();
		if pair.len() != 2 || pair[1] != reference || !oid(pair[0]) {
			return Err("remote ref response malformed".into());
		}
		Ok(Some(pair[0].into()))
	}
	/// Fills the push fields a preview leaves out: remote and ref from the
	/// checked-out branch's upstream, commit = the newest first-parent commit
	/// this session integrated, expected_remote_head = what the remote answers
	/// now. Explicit fields win; `binding` validates the result either way.
	/// The commit scan stops after 200 first-parent commits; older integrates need
	/// an explicit `commit`.
	fn derive(&self, session: &str, input: &Value) -> Result<Value> {
		let mut out = input.clone();
		if input.get("remote").is_none() || input.get("ref").is_none() {
			let branch = self
				.git(&["symbolic-ref", "--quiet", "--short", "HEAD"])
				.map_err(|_| "no checked-out branch to derive remote and ref from; pass them")?;
			let branch = branch.trim();
			for (key, field) in [("remote", "remote"), ("ref", "merge")] {
				if input.get(key).is_none() {
					let value = self
						.git(&["config", "--get", &format!("branch.{branch}.{field}")])
						.map_err(|_| {
							format!("branch {branch} has no upstream {field}; pass {key}")
						})?;
					out[key] = json!(value.trim());
				}
			}
		}
		if input.get("commit").is_none() {
			let log = self.git(&[
				"log",
				"--first-parent",
				"-n",
				"200",
				"--format=%H%x00%B%x1e",
				"HEAD",
			])?;
			let integrated = log.split('\x1e').find_map(|entry| {
				let (sha, body) = entry.trim_start_matches('\n').split_once('\0')?;
				reviewed_trailers(body, session).then(|| sha.to_owned())
			});
			out["commit"] = json!(integrated.ok_or(
				"no commit this session integrated in the last 200 first-parent commits; integrate first, or pass commit"
			)?);
		}
		if input.get("expected_remote_head").is_none() {
			let remote = out["remote"].as_str().unwrap_or("");
			let reference = out["ref"].as_str().unwrap_or("");
			if !name(remote, 64) || !reference.starts_with("refs/heads/") {
				return Err("invalid push binding: remote must be a plain remote name, ref a full refs/heads/<branch>".into());
			}
			out["expected_remote_head"] = json!(self.remote_head(&self.url(remote)?, reference)?);
		}
		Ok(out)
	}
	fn binding(&self, session: &str, input: &Value, timeout_ms: u64) -> Result<(Binding, String)> {
		let get = |key: &str| {
			input[key]
				.as_str()
				.ok_or_else(|| format!("push {key} must be a string"))
		};
		let remote = get("remote")?;
		let reference = get("ref")?;
		let commit = get("commit")?;
		let expected = match input.get("expected_remote_head") {
			Some(Value::Null) => None,
			Some(Value::String(s)) if oid(s) => Some(s.clone()),
			_ => return Err("expected_remote_head must be a full OID or null".into()),
		};
		if !name(remote, 64)
			|| !reference.starts_with("refs/heads/")
			|| reference.len() > 256
			|| !reference
				.bytes()
				.all(|c| c.is_ascii_alphanumeric() || b"/_.-".contains(&c))
			|| !oid(commit)
		{
			return Err("invalid push binding: remote must be a plain remote name, ref a full refs/heads/<branch> of [A-Za-z0-9/_.-], commit a full 40-hex sha".into());
		}
		let mut b = Binding {
			repository: String::new(),
			session: session.into(),
			remote: remote.into(),
			reference: reference.into(),
			commit: commit.into(),
			expected_remote_head: expected,
			url_digest: "0".repeat(64),
			timeout_ms,
		};
		b.repository = "0".repeat(64);
		if !valid_binding(&b) {
			return Err("invalid bounded push binding".into());
		}
		self.git(&["check-ref-format", reference])?;
		if self.git(&["cat-file", "-t", commit])?.trim() != "commit" {
			return Err("reviewed local commit required".into());
		}
		let body = self.git(&["show", "-s", "--format=%B", commit])?;
		if !reviewed_trailers(&body, session) {
			return Err("commit lacks this session's terminal reviewed publication receipt (the Integrated-by, Gitfs-Session, Gitfs-Review, Gitfs-Consumed trailers `integrate` writes): push publishes only a commit `integrate` made on this same session; integrate first, then push the sha it answered".into());
		}

		if let Some(old) = &b.expected_remote_head {
			if old.len() != commit.len()
				|| self.git(&["cat-file", "-t", old])?.trim() != "commit"
				|| !self
					.run(self.command(&["merge-base", "--is-ancestor", old, commit]))?
					.success
			{
				return Err(
					"expected remote head must be a known ancestor; no history rewrite".into(),
				);
			}
		}
		let url = self.url(remote)?;
		b.url_digest = digest(url.as_bytes());
		b.repository = self.repository()?;
		Ok((b, url))
	}
	fn lock(&self) -> Result<File> {
		private_dir(&self.dir)?;
		let f = OpenOptions::new()
			.create(true)
			.truncate(false)
			.read(true)
			.write(true)
			.open(self.dir.join("lock"))
			.map_err(|_| "push store lock unavailable")?;
		loop {
			self.check()?;
			if f.try_lock().is_ok() {
				return Ok(f);
			}
			std::thread::sleep(Duration::from_millis(2));
		}
	}
	fn records(&self) -> Result<Vec<(PathBuf, Record)>> {
		let mut records = Vec::new();
		for entry in fs::read_dir(&self.dir).map_err(|_| "push store unavailable")? {
			let entry = entry.map_err(|_| "push store unreadable")?;
			if entry.file_name() == "lock" {
				continue;
			}
			if records.len() >= capacity() {
				return Err("push attempt capacity reached".into());
			}
			let key = entry.file_name().to_string_lossy().into_owned();
			if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
				return Err("push store contains unrecognized evidence".into());
			}
			let bytes = bounded_read(&entry.path().join("record.json"), record_limit())?;
			let r: Record = serde_json::from_slice(&bytes).map_err(|_| "push record malformed")?;
			if !r.valid() || key != record_key(&r.binding.session, &r.operation_id) {
				return Err("push record invalid".into());
			}
			records.push((entry.path(), r));
		}
		Ok(records)
	}
	fn save(&self, dir: &Path, r: &Record) -> Result<()> {
		let bytes = serde_json::to_vec(r).map_err(|_| "push record encoding failed")?;
		if bytes.len() > record_limit() {
			return Err("push record exceeds limit".into());
		}
		atomic_write(&dir.join("record.json"), &bytes)
	}
	fn persist_readback(&self, dir: &Path, mut record: Record) -> Result<Record> {
		let Ok(_lock) = self.lock() else {
			// Without a durable update, return the existing conservative receipt.
			return serde_json::from_slice(&bounded_read(
				&dir.join("record.json"),
				record_limit(),
			)?)
			.map_err(|_| "push record malformed".into());
		};
		let prior: Record =
			serde_json::from_slice(&bounded_read(&dir.join("record.json"), record_limit())?)
				.map_err(|_| "push record malformed")?;
		if !prior.valid() || prior.revision != record.revision {
			return Err("push record changed unexpectedly".into());
		}
		if prior.status != Status::Unknown {
			record = prior;
		}
		self.save(dir, &record)?;
		Ok(record)
	}
	fn reconcile(&self, dir: &Path, mut r: Record) -> Result<Record> {
		if r.status != Status::Unknown {
			return Ok(r);
		}
		if bounded_read(&dir.join("hook-result"), 16).ok().as_deref() == Some(b"refused") {
			r.status = Status::Refused;
			r.reason = "hook_refused".into();
			return self.persist_readback(dir, r);
		}
		let read = (|| -> Result<Option<String>> {
			if self.repository()? != r.binding.repository {
				return Err("repository changed".into());
			}
			let url = self.url(&r.binding.remote)?;
			if digest(url.as_bytes()) != r.binding.url_digest {
				return Err("remote binding changed".into());
			}
			self.remote_head(&url, &r.binding.reference)
		})();
		match read {
			Ok(Some(head)) if head == r.binding.commit => {
				r.status = Status::Confirmed;
				r.reason = "exact_remote_confirmed".into();
			}
			Ok(_) => r.reason = "remote_not_confirmed".into(),
			Err(_) => r.reason = "binding_unavailable".into(),
		}
		self.persist_readback(dir, r)
	}
}

fn reviewed_trailers(body: &str, session: &str) -> bool {
	// Local integrate emits this exact terminal block. Receipt-looking prose elsewhere
	// is not a publication receipt, and duplicate/conflicting trailers are refused.
	let Some((_, block)) = body.trim_end_matches('\n').rsplit_once("\n\n") else {
		return false;
	};
	let lines: Vec<_> = block.lines().collect();
	let hex = |s: &str, len: usize| {
		s.len() == len
			&& s.bytes()
				.all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
	};
	lines.len() == 4
		&& lines[0] == "Integrated-by: gitfs"
		&& lines[1] == format!("Gitfs-Session: {session}")
		&& lines[2]
			.strip_prefix("Gitfs-Review: ")
			.is_some_and(|s| hex(s, 64))
		&& lines[3]
			.strip_prefix("Gitfs-Consumed: ")
			.is_some_and(|s| hex(s, 40) || hex(s, 64))
}

fn git_path(output: &str) -> Result<PathBuf> {
	let path = output
		.strip_suffix('\n')
		.ok_or("Git path response malformed")?;
	if path.is_empty() {
		return Err("Git path unavailable".into());
	}
	Ok(PathBuf::from(path))
}

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>> {
	let mut bytes = Vec::new();
	File::open(path)
		.map_err(|_| "push evidence unavailable")?
		.take(limit as u64 + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| "push evidence unreadable")?;
	if bytes.len() > limit {
		return Err("push evidence exceeds limit".into());
	}
	Ok(bytes)
}
fn private_dir(path: &Path) -> Result<()> {
	fs::create_dir_all(path).map_err(|_| "push store unavailable")?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(path, fs::Permissions::from_mode(0o700))
			.map_err(|_| "push store permissions unavailable")?;
	}
	Ok(())
}
pub(crate) use crate::hook::atomic_write;
#[cfg(test)]
pub(crate) use crate::hook::hook_valid;

pub fn execute(
	config: &StoreConfig,
	cwd: &Path,
	session: &str,
	input: &Value,
	timeout_ms: u64,
	deadline: Instant,
	cancel: &AtomicBool,
) -> Result<Value> {
	if !session_name(session) {
		return Err("bounded trusted session required".into());
	}
	if !(100..=60000).contains(&timeout_ms) {
		return Err("invalid push timeout".into());
	}
	let mut cx = Context {
		root: cwd.into(),
		dir: PathBuf::new(),
		deadline,
		cancel,
	};
	cx.root = git_path(&cx.git(&["rev-parse", "--show-toplevel"])?)?
		.canonicalize()
		.map_err(|_| "repository unavailable")?;
	cx.dir = cx.root.join(config.directory()).join("push-attempts");
	let phase = input["phase"].as_str().unwrap_or("preview");
	if !["preview", "commit", "reconcile"].contains(&phase) {
		return Err("unknown push phase".into());
	}
	let id = input["operation_id"].as_str().unwrap_or("");
	if phase != "preview" && !name(id, 64) {
		return Err(format!("{phase} needs operation_id: a name you pick for this push ([A-Za-z0-9_.-], at most 64), the same on a retry or reconcile so the push happens once; plus expected_revision from the preview"));
	}
	if phase == "preview"
		&& (input.get("operation_id").is_some() || input.get("expected_revision").is_some())
	{
		return Err("preview accepts no operation_id or expected_revision; to push, send them with phase: \"commit\"".into());
	}
	if phase != "preview" {
		let _lock = cx.lock()?;
		let records = cx.records()?;
		if let Some((dir, record)) = records
			.into_iter()
			.find(|(_, r)| r.operation_id == id && r.binding.session == session)
		{
			if phase == "commit" {
				if input["expected_revision"].as_str() != Some(&record.revision)
					|| input["remote"].as_str() != Some(&record.binding.remote)
					|| input["ref"].as_str() != Some(&record.binding.reference)
					|| input["commit"].as_str() != Some(&record.binding.commit)
					|| input["expected_remote_head"] != json!(record.binding.expected_remote_head)
				{
					return Err("operation identity binding conflict".into());
				}
				return Ok(record.value());
			}
			drop(_lock);
			return Ok(cx.reconcile(&dir, record)?.value());
		}
		if phase == "reconcile" {
			return Err("push attempt unavailable".into());
		}
	}
	let derived;
	let input = if phase == "preview" {
		derived = cx.derive(session, input)?;
		&derived
	} else {
		input
	};
	let (binding, url) = cx.binding(session, input, timeout_ms)?;
	crate::review::check_range(
		&cx.root,
		binding
			.expected_remote_head
			.as_deref()
			.unwrap_or(&"0".repeat(binding.commit.len())),
		&binding.commit,
	)?;
	let revision = revision(&binding);
	let remote = cx.remote_head(&url, &binding.reference)?;
	if remote != binding.expected_remote_head {
		return Err("expected remote head changed; preview again".into());
	}
	if phase == "preview" {
		return Ok(
			json!({"phase":"preview","revision":revision,"binding":binding,"already_present":remote.as_deref()==Some(&binding.commit),"next":"push again with the same fields plus phase: commit, expected_revision: <revision> and operation_id: a name you pick ([A-Za-z0-9_.-]); reuse it to retry or reconcile"}),
		);
	}
	if input["expected_revision"].as_str() != Some(&revision) {
		return Err("reviewed push binding changed; preview again".into());
	}
	let dir = cx.dir.join(record_key(session, id));
	let mut record = Record {
		schema: "gitfs-push/v1".into(),
		operation_id: id.into(),
		revision,
		binding,
		status: Status::Unknown,
		reason: "dispatch_pending_or_unknown".into(),
	};
	{
		let _lock = cx.lock()?;
		let records = cx.records()?;
		if let Some((_, prior)) = records
			.iter()
			.find(|(_, r)| r.operation_id == id && r.binding.session == session)
		{
			if prior.revision != record.revision {
				return Err("operation identity binding conflict".into());
			}
			return Ok(prior.value());
		}

		if records.len() >= capacity() {
			return Err("push attempt capacity reached".into());
		}
		if records.iter().any(|(_, r)| {
			r.status == Status::Unknown
				&& r.binding.url_digest == record.binding.url_digest
				&& r.binding.reference == record.binding.reference
		}) {
			return Err("destination has an unresolved attempt; reconcile it first".into());
		}
		private_dir(&dir)?;
		cx.save(&dir, &record)?;
		File::open(&cx.dir)
			.and_then(|f| f.sync_all())
			.map_err(|_| "push reservation durability failed")?;
	}
	if remote.as_deref() == Some(&record.binding.commit) {
		record.status = Status::Confirmed;
		record.reason = "exact_remote_confirmed".into();
		let _lock = cx.lock()?;
		cx.save(&dir, &record)?;
		return Ok(record.value());
	}
	// On Windows the not(unix) arm returns first, leaving the Unix dispatch
	// body unreachable; the code stays for the platforms that run it.
	#[cfg_attr(not(unix), allow(unreachable_code))]
	let dispatch = (|| -> Result<Output> {
		let hooks = dir.join("hooks");
		private_dir(&hooks)?;
		#[cfg(unix)]
		std::os::unix::fs::symlink(crate::hook_program()?, hooks.join("pre-push"))
			.map_err(|_| "owned push hook unavailable")?;
		#[cfg(not(unix))]
		return Err("owned push hook unsupported".into());
		let hook = hooks.to_str().ok_or("hook path invalid")?;
		let mut command = cx.command(&[
			"-c",
			&format!("core.hooksPath={hook}"),
			"push",
			"--porcelain",
			"--no-follow-tags",
			"--recurse-submodules=no",
			"--no-mirror",
			"--",
			&url,
			&format!("{}:{}", record.binding.commit, record.binding.reference),
		]);
		command
			.env("GITFS_PUSH_HOOK", "1")
			.env("GITFS_PUSH_NEW", &record.binding.commit)
			.env(
				"GITFS_PUSH_OLD",
				record
					.binding
					.expected_remote_head
					.clone()
					.unwrap_or_else(|| "0".repeat(record.binding.commit.len())),
			)
			.env("GITFS_PUSH_REF", &record.binding.reference)
			.env("GITFS_REVIEW_ROOT", &cx.root)
			.env("GITFS_PUSH_RESULT", dir.join("hook-result"));
		cx.run(command)
	})();

	let _ = dispatch; // Every unacknowledged post-dispatch outcome remains unknown.
	Ok(cx.reconcile(&dir, record)?.value())
}

#[cfg(test)]
#[path = "../tests/unit/push/tests.rs"]
mod tests;
