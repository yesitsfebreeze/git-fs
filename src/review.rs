//! Repository-local review policy and durable evidence for publication.
//!
//! Local configuration and receipts are workflow controls, not a sandbox against
//! a process that can edit .git. Candidate source never selects its reviewer.

use crate::{store::StoreConfig, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const OUTPUT_LIMIT: usize = 128 * 1024;
const DIFF_LIMIT: usize = 4 * 1024 * 1024;

fn digest(bytes: &[u8]) -> String {
	format!("{:x}", Sha256::digest(bytes))
}

fn git(root: &Path, args: &[&str]) -> Command {
	let mut command = Command::new("git");
	command
		.arg("-C")
		.arg(root)
		.args(args)
		.env("GIT_NO_REPLACE_OBJECTS", "1");
	for key in [
		"GIT_DIR",
		"GIT_WORK_TREE",
		"GIT_COMMON_DIR",
		"GIT_INDEX_FILE",
		"GIT_CONFIG",
		"GIT_CONFIG_COUNT",
		"GIT_CONFIG_PARAMETERS",
		"GIT_OBJECT_DIRECTORY",
		"GIT_ALTERNATE_OBJECT_DIRECTORIES",
	] {
		command.env_remove(key);
	}
	command
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
	let out = git(root, args)
		.output()
		.map_err(|e| format!("review Git unavailable: {e}"))?;
	if !out.status.success() {
		return Err("review Git lookup failed".into());
	}
	String::from_utf8(out.stdout).map_err(|_| "review Git output is not UTF-8".into())
}

fn local_setting(root: &Path, name: &str) -> Result<Option<String>> {
	let out = git(
		root,
		&["config", "--local", "--no-includes", "--get-all", name],
	)
	.output()
	.map_err(|e| format!("review configuration unavailable: {e}"))?;
	if out.status.code() == Some(1) && out.stdout.is_empty() {
		return Ok(None);
	}
	if !out.status.success() || out.stdout.len() > 32768 {
		return Err(format!("invalid local {name}"));
	}
	let value = String::from_utf8(out.stdout).map_err(|_| format!("invalid local {name}"))?;
	let lines: Vec<_> = value.lines().collect();
	if lines.len() != 1 {
		return Err(format!("local {name} must have exactly one value"));
	}
	Ok(Some(lines[0].to_owned()))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
	argv: Vec<String>,
	timeout_ms: u64,
	files: Vec<(String, String)>,
}

pub(crate) fn policy(root: &Path) -> Result<Option<Policy>> {
	let Some(command) = local_setting(root, "gitfs.reviewCommand")? else {
		return Ok(None);
	};
	let argv: Vec<String> = serde_json::from_str(&command)
		.map_err(|_| "gitfs.reviewCommand must be a JSON argv array (no shell)")?;
	if argv.is_empty()
		|| argv.len() > 32
		|| !Path::new(&argv[0]).is_absolute()
		|| argv
			.iter()
			.any(|arg| arg.contains('\0') || arg.len() > 4096)
	{
		return Err("gitfs.reviewCommand requires an absolute executable and bounded argv".into());
	}
	let timeout_ms = match local_setting(root, "gitfs.reviewTimeoutMs")? {
		Some(value) => value
			.parse::<u64>()
			.map_err(|_| "invalid gitfs.reviewTimeoutMs")?,
		None => 300_000,
	};
	if !(1..=3_600_000).contains(&timeout_ms) {
		return Err("gitfs.reviewTimeoutMs must be between 1 and 3600000".into());
	}
	let mut files = Vec::new();
	for (index, arg) in argv.iter().enumerate() {
		let path = Path::new(arg);
		if path.is_absolute() && (index == 0 || path.is_file()) {
			files.push((arg.clone(), file_hash(path)?));
		}
	}
	Ok(Some(Policy {
		argv,
		timeout_ms,
		files,
	}))
}

fn open_regular(path: &Path) -> Result<File> {
	let mut options = OpenOptions::new();
	options.read(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		options.custom_flags(libc::O_NONBLOCK);
	}
	let file = options
		.open(path)
		.map_err(|e| format!("review evidence unavailable at {}: {e}", path.display()))?;
	if !file
		.metadata()
		.map_err(|_| "review file metadata unavailable")?
		.is_file()
	{
		return Err("review policy and evidence require regular files".into());
	}
	Ok(file)
}

fn file_hash(path: &Path) -> Result<String> {
	const LIMIT: usize = 512 * 1024 * 1024;
	let mut file = open_regular(path)?;
	let mut hash = Sha256::new();
	let mut bytes = [0; 64 * 1024];
	let mut total = 0;
	loop {
		let count = file
			.read(&mut bytes)
			.map_err(|_| "review command file unreadable")?;
		if count == 0 {
			break;
		}
		total += count;
		if total > LIMIT {
			return Err("review command file exceeds 512 MiB".into());
		}
		hash.update(&bytes[..count]);
	}
	Ok(format!("{:x}", hash.finalize()))
}

fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>> {
	let mut bytes = Vec::new();
	open_regular(path)?
		.take(limit as u64 + 1)
		.read_to_end(&mut bytes)
		.map_err(|_| "review evidence unreadable")?;
	if bytes.len() > limit {
		return Err("review evidence exceeds limit".into());
	}
	Ok(bytes)
}

fn directory(root: &Path) -> Result<PathBuf> {
	let dir = git_text(root, &["rev-parse", "--absolute-git-dir"])?;
	Ok(PathBuf::from(dir.trim_end_matches('\n')).join("gitfs-review"))
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rounds {
	held: Vec<String>,
	resets: Vec<Value>,
}

struct RoundLock {
	_file: File,
	path: PathBuf,
	rounds: Rounds,
}

impl RoundLock {
	fn acquire(root: &Path, session: &str) -> Result<Self> {
		let dir = directory(root)?;
		fs::create_dir_all(&dir).map_err(|e| format!("review directory unavailable: {e}"))?;
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
				.map_err(|_| "review directory permissions unavailable")?;
		}
		let key = digest(session.as_bytes());
		let file = OpenOptions::new()
			.create(true)
			.truncate(false)
			.read(true)
			.write(true)
			.open(dir.join(format!("rounds-{key}.lock")))
			.map_err(|_| "review lock unavailable")?;
		match file.try_lock() {
			Ok(()) => {}
			Err(std::fs::TryLockError::WouldBlock) => {
				return Err("a review is already running for this session".into())
			}
			Err(std::fs::TryLockError::Error(e)) => {
				return Err(format!("review lock unavailable: {e}"))
			}
		}
		let path = dir.join(format!("rounds-{key}.json"));
		let rounds: Rounds = if path.exists() {
			serde_json::from_slice(&bounded_read(&path, OUTPUT_LIMIT)?)
				.map_err(|_| "review round history malformed")?
		} else {
			Rounds::default()
		};
		if rounds.held.len() > 3 || rounds.resets.len() > 32 {
			return Err("review round history exceeds limit".into());
		}
		Ok(Self {
			_file: file,
			path,
			rounds,
		})
	}

	fn save(&self) -> Result<()> {
		crate::hook::atomic_write(
			&self.path,
			&serde_json::to_vec(&self.rounds).map_err(|e| e.to_string())?,
		)
	}
}

/// Start a new bounded review cycle after explicitly reconsidering the approach.
/// The reason and prior held revisions remain in local history.
pub fn reset_rounds(root: &Path, session: &str, reason: &str) -> Result<()> {
	if session.is_empty() || session.len() > 128 || reason.trim().is_empty() || reason.len() > 2048
	{
		return Err(
			"review-reset requires a session and a nonempty reason (at most 2048 bytes)".into(),
		);
	}
	let mut lock = RoundLock::acquire(root, session)?;
	if lock.rounds.resets.len() >= 32 {
		return Err("review reset history is full; reassess outside this session".into());
	}
	lock.rounds
		.resets
		.push(json!({"reason":reason,"held":lock.rounds.held}));
	lock.rounds.held.clear();
	lock.save()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Finding {
	id: String,
	severity: String,
	path: String,
	line: u64,
	evidence: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verdict {
	approved: bool,
	summary: String,
	findings: Vec<Finding>,
}

impl Verdict {
	fn validate(&self) -> Result<()> {
		if self.summary.trim().is_empty() || self.summary.len() > 8192 || self.findings.len() > 128
		{
			return Err("review verdict needs a bounded summary and findings".into());
		}
		let mut ids = std::collections::HashSet::new();
		for finding in &self.findings {
			if finding.id.is_empty()
				|| finding.id.len() > 256
				|| !ids.insert(&finding.id)
				|| !["blocker", "high", "medium", "low"].contains(&finding.severity.as_str())
				|| finding.path.is_empty()
				|| finding.path.len() > 4096
				|| Path::new(&finding.path)
					.components()
					.any(|part| !matches!(part, Component::Normal(_)))
				|| finding.line == 0
				|| finding.evidence.trim().is_empty()
				|| finding.evidence.len() > 8192
			{
				return Err("review finding requires a unique ID, severity, relative path, positive line and evidence".into());
			}
		}
		if self.approved
			&& self
				.findings
				.iter()
				.any(|finding| finding.severity != "low")
		{
			return Err("review cannot approve unresolved blocking findings".into());
		}
		Ok(())
	}
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
	schema: String,
	review_revision: String,
	preview: Value,
	verdict: Verdict,
}

#[cfg(unix)]
struct ProcessGroup(u32);
#[cfg(unix)]
impl Drop for ProcessGroup {
	fn drop(&mut self) {
		// Descendants may retain pipes after the reviewer exits.
		unsafe {
			libc::kill(-(self.0 as i32), libc::SIGKILL);
		}
	}
}

pub(crate) async fn run(
	root: &Path,
	configured: &Policy,
	preview: &Value,
	revision: &str,
	diff: &str,
	cancel: Arc<AtomicBool>,
) -> Result<()> {
	if diff.len() > DIFF_LIMIT {
		return Err("review diff exceeds 4 MiB; split the change".into());
	}
	if policy(root)?.as_ref() != Some(configured) {
		return Err("review policy changed; preview again".into());
	}
	let session = preview["session"]
		.as_str()
		.ok_or("review session missing")?;
	let mut rounds = RoundLock::acquire(root, session)?;
	if rounds.rounds.held.len() >= 3 {
		return Err(format!("review held three rounds; reconsider the approach, then run git-fs review-reset {session} --reason '<what changed>'"));
	}
	let request = json!({"schema":"gitfs-review-request/v1", "repo":root,
		"session":preview["session"], "base":preview["source"]["head"],
		"tree":preview["source"]["tree"], "review_revision":revision,
		"diff":diff, "intent":preview["subject"]});
	let mut command = tokio::process::Command::new(&configured.argv[0]);
	command
		.args(&configured.argv[1..])
		.current_dir(root)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.kill_on_drop(true);
	#[cfg(unix)]
	command.process_group(0);
	let mut child = command
		.spawn()
		.map_err(|e| format!("review command could not start: {e}"))?;
	#[cfg(unix)]
	let group = ProcessGroup(child.id().ok_or("review process missing")?);
	let mut stdin = child.stdin.take().ok_or("review stdin missing")?;
	let stdout = child.stdout.take().ok_or("review stdout missing")?;
	let stderr = child.stderr.take().ok_or("review stderr missing")?;
	let exchange = async {
		let write = async {
			stdin
				.write_all(&serde_json::to_vec(&request).map_err(|e| e.to_string())?)
				.await
				.map_err(|_| "review command closed its input")?;
			drop(stdin);
			Ok::<_, String>(())
		};
		let read = |pipe: Box<dyn tokio::io::AsyncRead + Unpin + Send>| async move {
			let mut bytes = Vec::new();
			pipe.take(OUTPUT_LIMIT as u64 + 1)
				.read_to_end(&mut bytes)
				.await
				.map_err(|_| "review output interrupted")?;
			if bytes.len() > OUTPUT_LIMIT {
				return Err("review output exceeds 128 KiB".into());
			}
			Ok::<_, String>(bytes)
		};
		let wait = async {
			child
				.wait()
				.await
				.map_err(|_| "review exit status unavailable".to_string())
		};
		let (_, out, err, status) =
			tokio::try_join!(write, read(Box::new(stdout)), read(Box::new(stderr)), wait)?;
		if !status.success() {
			let detail: String = String::from_utf8_lossy(&err)
				.chars()
				.take(2000)
				.map(|c| if c.is_control() { ' ' } else { c })
				.collect();
			return Err(format!("review command failed ({status}): {detail}"));
		}
		Ok(out)
	};
	let cancelled = async {
		loop {
			if cancel.load(Ordering::SeqCst) {
				break;
			}
			tokio::time::sleep(Duration::from_millis(25)).await;
		}
	};
	let result = tokio::select! {
		result = tokio::time::timeout(Duration::from_millis(configured.timeout_ms), exchange) =>
			result.map_err(|_| "review command timed out; no commit published".to_string()).and_then(|value| value),
		_ = cancelled => Err("review cancelled; no commit published".into()),
	};
	#[cfg(unix)]
	drop(group);
	let _ = child.wait().await;
	let bytes = result?;
	let verdict: Verdict = serde_json::from_slice(&bytes)
		.map_err(|e| format!("review verdict is malformed JSON: {e}"))?;
	verdict.validate()?;
	if policy(root)?.as_ref() != Some(configured) {
		return Err("review policy changed during review; preview again".into());
	}
	let summary = verdict.summary.clone();
	let approved = verdict.approved;
	let receipt = Receipt {
		schema: "gitfs-review-receipt/v1".into(),
		review_revision: revision.into(),
		preview: preview.clone(),
		verdict,
	};
	let dir = directory(root)?;
	fs::create_dir_all(&dir).map_err(|e| format!("review evidence directory unavailable: {e}"))?;
	crate::hook::atomic_write(
		&dir.join(format!("{revision}.json")),
		&serde_json::to_vec(&receipt).map_err(|e| e.to_string())?,
	)?;
	if approved {
		rounds.rounds.held.clear();
	} else {
		rounds.rounds.held.push(revision.into());
	}
	rounds.save()?;
	if !approved {
		return Err(format!(
			"review held publication: {summary}; findings: {}",
			serde_json::to_string(&receipt.verdict.findings).unwrap_or_default()
		));
	}
	Ok(())
}

/// Verify the exact integrated commit against current repository policy. No
/// policy preserves the existing behavior. This does not attest test execution.
pub fn check_commit(root: &Path, commit: &str) -> Result<()> {
	let Some(configured) = policy(root)? else {
		return Ok(());
	};
	check_commit_with_policy(root, commit, &configured)
}

fn check_commit_with_policy(root: &Path, commit: &str, configured: &Policy) -> Result<()> {
	if ![40, 64].contains(&commit.len()) || !commit.bytes().all(|c| c.is_ascii_hexdigit()) {
		return Err("review-check requires a full commit OID".into());
	}
	let text = git_text(root, &["show", "-s", "--format=%T%n%P%n%B", commit])?;
	let mut lines = text.lines();
	let tree = lines.next().ok_or("commit tree missing")?;
	let parents: Vec<_> = lines
		.next()
		.ok_or("commit parent missing")?
		.split_whitespace()
		.collect();
	if parents.len() != 1 {
		return Err("reviewed publication requires exactly one parent".into());
	}
	let body = lines.collect::<Vec<_>>().join("\n");
	let (_, trailers) = body
		.trim_end()
		.rsplit_once("\n\n")
		.ok_or("commit has no review receipt; use git-fs integrate")?;
	let trailers: Vec<_> = trailers.lines().collect();
	if trailers.len() != 4 || trailers[0] != "Integrated-by: gitfs" {
		return Err("commit has no integration review receipt".into());
	}
	let session = trailers[1]
		.strip_prefix("Gitfs-Session: ")
		.ok_or("review session missing")?;
	let revision = trailers[2]
		.strip_prefix("Gitfs-Review: ")
		.ok_or("review revision missing")?;
	let consumed = trailers[3]
		.strip_prefix("Gitfs-Consumed: ")
		.ok_or("review overlay missing")?;
	if revision.len() != 64 || !revision.bytes().all(|c| c.is_ascii_hexdigit()) {
		return Err("review revision invalid".into());
	}
	let bytes = bounded_read(
		&directory(root)?.join(format!("{revision}.json")),
		2 * 1024 * 1024,
	)?;
	let receipt: Receipt =
		serde_json::from_slice(&bytes).map_err(|_| "review receipt malformed")?;
	receipt.verdict.validate()?;
	let preview = &receipt.preview;
	let repository = StoreConfig::new(".git-fs")
		.at_readonly(root)?
		.repository_identity()?;
	if receipt.schema != "gitfs-review-receipt/v1"
		|| receipt.review_revision != revision
		|| digest(&serde_json::to_vec(preview).map_err(|e| e.to_string())?) != revision
		|| !receipt.verdict.approved
		|| preview["command_review"]
			!= serde_json::to_value(configured).map_err(|e| e.to_string())?
		|| preview["source"]["repository"] != repository
		|| preview["source"]["head"] != parents[0]
		|| preview["source"]["tree"] != tree
		|| preview["source"]["overlay"] != consumed
		|| preview["session"] != session
		|| preview["subject"] != body.lines().next().unwrap_or("")
	{
		return Err(
			"review receipt is stale or does not cover this commit and current policy".into(),
		);
	}
	Ok(())
}

/// Every newly published commit must carry its own current review evidence.
pub fn check_range(root: &Path, old: &str, new: &str) -> Result<()> {
	let Some(configured) = policy(root)? else {
		return Ok(());
	};
	let oid = |value: &str| {
		[40, 64].contains(&value.len()) && value.bytes().all(|c| c.is_ascii_hexdigit())
	};
	if !oid(old) || !oid(new) {
		return Err("review push requires full commit OIDs".into());
	}
	let range = if old.bytes().all(|c| c == b'0') {
		new.to_owned()
	} else {
		format!("{old}..{new}")
	};
	let commits = git_text(root, &["rev-list", "--max-count=257", &range])?;
	if commits.lines().count() > 256 {
		return Err("review push exceeds 256 commits; publish a smaller range".into());
	}
	for commit in commits.lines() {
		check_commit_with_policy(root, commit, &configured)?;
	}
	if policy(root)?.as_ref() != Some(&configured) {
		return Err("review policy changed while checking publication".into());
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn command_fingerprint_streams_large_files_and_refuses_nonregular_evidence() {
		use std::io::Write;
		let temp = tempfile::tempdir().unwrap();
		let path = temp.path().join("large-executable");
		let mut file = File::create(&path).unwrap();
		let chunk = [42; 64 * 1024];
		let mut expected = Sha256::new();
		for _ in 0..528 {
			file.write_all(&chunk).unwrap();
			expected.update(chunk);
		}
		drop(file);
		assert_eq!(
			file_hash(&path).unwrap(),
			format!("{:x}", expected.finalize())
		);
		assert!(bounded_read(temp.path(), 100)
			.unwrap_err()
			.contains("regular files"));
	}
}
