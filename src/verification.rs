//! Check receipts from a configured host-owned store, bound to one reviewed
//! integration candidate. Git-FS validates evidence; it does not execute a
//! second check runner or accept approval JSON from an agent's tool arguments.

use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const POLICY_SCHEMA: &str = "gitfs-verification-policy/v1";
const RECEIPT_SCHEMA: &str = "gitfs-verification-receipt/v1";
const MAX_RECEIPT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CheckDefinition {
	id: String,
	definition: String,
	required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Policy {
	schema: String,
	runner_id: String,
	receipt_dir: PathBuf,
	checks: Vec<CheckDefinition>,
	environment_allowlist: Vec<String>,
	reuse: String,
}

fn hex_digest(bytes: &[u8]) -> String {
	format!("{:x}", Sha256::digest(bytes))
}

fn json_digest<T: Serialize>(value: &T) -> Result<String> {
	// Match the canonical JSON object-key order used by preview/fixture JSON.
	let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
	Ok(hex_digest(
		&serde_json::to_vec(&value).map_err(|e| e.to_string())?,
	))
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
	value[field]
		.as_str()
		.filter(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control))
		.ok_or_else(|| format!("verification receipt requires {field}"))
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
	let output = Command::new("git")
		.arg("-C")
		.arg(root)
		.args(args)
		.output()
		.map_err(|e| format!("verification Git command unavailable: {e}"))?;
	if !output.status.success() {
		return Err(format!(
			"verification git {} failed: {}",
			args.join(" "),
			String::from_utf8_lossy(&output.stderr).trim()
		));
	}
	String::from_utf8(output.stdout)
		.map(|s| s.trim().to_owned())
		.map_err(|e| format!("verification Git output invalid: {e}"))
}

fn configured_policy(root: &Path) -> Result<Option<Policy>> {
	let output = Command::new("git")
		.arg("-C")
		.arg(root)
		.args(["config", "--local", "--get", "gitfs.verificationPolicy"])
		.output()
		.map_err(|e| format!("verification policy lookup unavailable: {e}"))?;
	if output.status.code() == Some(1) {
		return Ok(None);
	}
	if !output.status.success() {
		return Err(format!(
			"verification policy lookup failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		));
	}
	if output.stdout.len() > 64 * 1024 {
		return Err("verification policy exceeds 64 KiB".into());
	}
	let policy: Policy = serde_json::from_slice(&output.stdout)
		.map_err(|e| format!("verification policy invalid: {e}"))?;
	if policy.schema != POLICY_SCHEMA
		|| policy.reuse != "exact-candidate"
		|| policy.runner_id.is_empty()
		|| policy.runner_id.len() > 128
		|| !policy.receipt_dir.is_absolute()
		|| policy.checks.is_empty()
		|| policy.checks.len() > 32
	{
		return Err(
			"verification policy schema, runner, receipt directory or reuse invalid".into(),
		);
	}
	let mut names = HashSet::new();
	for check in &policy.checks {
		if !names.insert(&check.id)
			|| check.id.is_empty()
			|| check.id.len() > 128
			|| check.definition.is_empty()
			|| check.definition.len() > 4096
			|| !check.required
		{
			return Err("verification check definitions invalid or duplicated".into());
		}
	}
	let mut env = HashSet::new();
	for key in &policy.environment_allowlist {
		if key.is_empty() || key.len() > 128 || !env.insert(key) {
			return Err("verification environment allowlist invalid or duplicated".into());
		}
	}
	let directory_meta = fs::symlink_metadata(&policy.receipt_dir)
		.map_err(|e| format!("trusted verification receipt directory unavailable: {e}"))?;
	if directory_meta.file_type().is_symlink() || !directory_meta.is_dir() {
		return Err("trusted verification receipt directory must be a real directory".into());
	}
	let trusted = policy
		.receipt_dir
		.canonicalize()
		.map_err(|e| e.to_string())?;
	let repository = root.canonicalize().map_err(|e| e.to_string())?;
	if trusted.starts_with(&repository) {
		return Err("trusted verification receipt directory must be outside the repository".into());
	}
	Ok(Some(policy))
}

/// This is a read-only description of the configured *requirements*, not an
/// assertion that any command ran or passed. The preview hash incorporates it.
pub(crate) fn policy_fingerprint(root: &Path) -> Result<Option<Value>> {
	let Some(policy) = configured_policy(root)? else {
		return Ok(None);
	};
	let digest = json_digest(&policy)?;
	let definitions = json_digest(&policy.checks)?;
	Ok(Some(json!({
		"schema": POLICY_SCHEMA,
		"policy_digest": digest,
		"check_definition_digest": definitions,
		"runner_id": policy.runner_id,
		"reuse": policy.reuse,
	})))
}

fn hex_oid(value: &str, length: usize) -> bool {
	value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn bounded_regular(path: &Path, max: u64, root: &Path) -> Result<Vec<u8>> {
	let metadata = fs::symlink_metadata(path)
		.map_err(|e| format!("verification artifact unavailable: {e}"))?;
	if !metadata.file_type().is_file() || metadata.file_type().is_symlink() || metadata.len() > max
	{
		return Err("verification artifact must be a bounded regular file".into());
	}
	let canonical = path.canonicalize().map_err(|e| e.to_string())?;
	let trusted = root.canonicalize().map_err(|e| e.to_string())?;
	if canonical.parent() != Some(trusted.as_path()) {
		return Err("verification artifact escapes trusted receipt directory".into());
	}
	let mut bytes = Vec::new();
	File::open(path)
		.map_err(|e| e.to_string())?
		.take(max + 1)
		.read_to_end(&mut bytes)
		.map_err(|e| e.to_string())?;
	if bytes.len() as u64 > max {
		return Err("verification artifact exceeds size limit".into());
	}
	Ok(bytes)
}

fn check_artifact(root: &Path, value: &Value) -> Result<()> {
	let name = required_string(value, "path")?;
	let hash = required_string(value, "sha256")?;
	if !hex_oid(hash, 64) || !Path::new(name).is_absolute() {
		return Err("verification artifact hash or path invalid".into());
	}
	if hex_digest(&bounded_regular(Path::new(name), MAX_LOG_BYTES, root)?) != hash {
		return Err("verification artifact bytes changed after the check".into());
	}
	Ok(())
}

fn validate_record(
	root: &Path,
	policy: &Policy,
	session: &str,
	source: &Value,
	revision: &str,
	check_current_head: bool,
) -> Result<Value> {
	if !hex_oid(revision, 64) {
		return Err("verification revision requires a full SHA-256 digest".into());
	}
	let path = policy.receipt_dir.join(format!("{revision}.json"));
	let raw = bounded_regular(&path, MAX_RECEIPT_BYTES, &policy.receipt_dir)?;
	let receipt: Value =
		serde_json::from_slice(&raw).map_err(|e| format!("verification receipt malformed: {e}"))?;
	let preview = &receipt["preview"];
	let expected_fingerprint = policy_fingerprint(root)?
		.ok_or("verification policy disappeared during receipt validation")?;
	let expected_policy = json_digest(policy)?;
	let expected_checks = json_digest(&policy.checks)?;
	if required_string(&receipt, "schema")? != RECEIPT_SCHEMA
		|| required_string(&receipt, "review_revision")? != revision
		|| required_string(&receipt, "policy_digest")? != expected_policy
		|| required_string(&receipt, "check_definition_digest")? != expected_checks
		|| required_string(preview, "schema")? != "gitfs-reviewed-commit/v1"
		|| required_string(preview, "session")? != session
		|| preview["source"] != *source
		|| preview["verification_policy"] != expected_fingerprint
		|| hex_digest(&serde_json::to_vec(preview).map_err(|e| e.to_string())?) != revision
		|| receipt["source"] != *source
		|| required_string(&receipt, "run_id")?.len() > 128
		|| required_string(&receipt, "session")? != session
		|| required_string(&receipt["runner"], "id")? != policy.runner_id
		|| required_string(&receipt["runner"], "revision")?.len() > 128
	{
		return Err("verification receipt does not bind the candidate, runner or policy".into());
	}
	for field in [
		"repository",
		"branch",
		"head",
		"index_revision",
		"overlay",
		"tree",
	] {
		required_string(source, field)?;
	}
	// The repository identity is independent of the caller-provided JSON.
	let repository = crate::store::StoreConfig::new(".git-fs")
		.at_readonly(root)?
		.repository_identity()?;
	if source["repository"] != repository {
		return Err("verification repository identity differs from current repository".into());
	}
	if check_current_head
		&& (source["head"] != git_text(root, &["rev-parse", "HEAD"])?
			|| source["branch"] != git_text(root, &["symbolic-ref", "--short", "HEAD"])?)
	{
		return Err("verification source HEAD or branch advanced; preview again".into());
	}
	for (object, field) in [
		(&receipt["platform"], "os"),
		(&receipt["platform"], "arch"),
		(&receipt["toolchain"], "rustc"),
		(&receipt["dependencies"], "lockfile"),
	] {
		required_string(object, field)?;
	}
	if !receipt["limitations"].as_array().is_some_and(|items| {
		!items.is_empty()
			&& items
				.iter()
				.all(|entry| entry.as_str().is_some_and(|s| !s.trim().is_empty()))
	}) {
		return Err("verification receipt must disclose limitations".into());
	}
	let recorded_environment = receipt["environment"]["allowlisted"]
		.as_object()
		.ok_or("verification allowlisted environment missing")?;
	if recorded_environment
		.iter()
		.any(|(name, value)| !policy.environment_allowlist.contains(name) || !value.is_string())
	{
		return Err("verification receipt includes an unallowlisted environment value".into());
	}
	let checks = receipt["checks"]
		.as_array()
		.ok_or("verification checks missing")?;
	if checks.len() != policy.checks.len() {
		return Err("verification checks do not match configured definitions".into());
	}
	let mut found = HashSet::new();
	for check in checks {
		let id = required_string(check, "id")?;
		let Some(definition) = policy.checks.iter().find(|expected| expected.id == id) else {
			return Err("verification check identity unknown".into());
		};
		if !found.insert(id) {
			return Err("verification check identity unknown or duplicated".into());
		}
		if required_string(check, "definition_digest")?
			!= hex_digest(definition.definition.as_bytes())
		{
			return Err(
				"verification executed check definition differs from configured policy".into(),
			);
		}
		if required_string(check, "status")? != "passed"
			|| required_string(check, "input_integrity")? != "unchanged"
		{
			return Err("verification check failed, cancelled or mutated source".into());
		}
		check_artifact(&policy.receipt_dir, &check["stdout"])?;
		check_artifact(&policy.receipt_dir, &check["stderr"])?;
		let outputs = check["output_digests"]
			.as_array()
			.ok_or("verification output digests missing")?;
		for output in outputs {
			check_artifact(&policy.receipt_dir, output)?;
		}
	}
	Ok(receipt)
}

/// Only a reference derived from the reviewed preview revision is accepted.
/// The receipt itself is read from a configured trusted runner-owned store;
/// no MCP operation accepts agent-provided approval JSON.
pub fn validate_candidate(
	root: &Path,
	session: &str,
	source: &Value,
	revision: &str,
) -> Result<Option<Value>> {
	let Some(policy) = configured_policy(root)? else {
		return Ok(None);
	};
	validate_record(root, &policy, session, source, revision, true).map(Some)
}

/// Push-hook validation binds a past reviewed source to the actual commit,
/// rather than mistakenly expecting the current trunk to remain at its parent.
pub(crate) fn check_commit(root: &Path, commit: &str) -> Result<()> {
	let Some(policy) = configured_policy(root)? else {
		return Ok(());
	};
	if ![40, 64].contains(&commit.len()) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
		return Err("verification commit requires full Git OID".into());
	}
	let text = git_text(root, &["show", "-s", "--format=%T%n%P%n%B", commit])?;
	let mut lines = text.lines();
	let tree = lines.next().ok_or("verification commit tree missing")?;
	let parents: Vec<_> = lines
		.next()
		.ok_or("verification commit parent missing")?
		.split_whitespace()
		.collect();
	if parents.len() != 1 {
		return Err("verification requires a one-parent integrated commit".into());
	}
	let body = lines.collect::<Vec<_>>().join("\n");
	let (_, trailers) = body
		.trim_end()
		.rsplit_once("\n\n")
		.ok_or("verification commit lacks integrated receipt")?;
	let trailers: Vec<_> = trailers.lines().collect();
	if trailers.len() != 4 || trailers[0] != "Integrated-by: gitfs" {
		return Err("verification commit lacks Git-FS integration trailer".into());
	}
	let session = trailers[1]
		.strip_prefix("Gitfs-Session: ")
		.ok_or("verification commit session missing")?;
	let revision = trailers[2]
		.strip_prefix("Gitfs-Review: ")
		.ok_or("verification review revision missing")?;
	let consumed = trailers[3]
		.strip_prefix("Gitfs-Consumed: ")
		.ok_or("verification overlay identity missing")?;
	if !hex_oid(revision, 64) {
		return Err("verification review revision malformed".into());
	}
	let receipt_path = policy.receipt_dir.join(format!("{revision}.json"));
	let raw = bounded_regular(&receipt_path, MAX_RECEIPT_BYTES, &policy.receipt_dir)?;
	let receipt: Value =
		serde_json::from_slice(&raw).map_err(|_| "verification publication receipt malformed")?;
	let source = &receipt["source"];
	let preview = &receipt["preview"];
	let git_author = git_text(root, &["show", "-s", "--format=%an%n%ae%n%cn%n%ce", commit])?;
	let actual: Vec<_> = git_author.lines().collect();
	if source["head"] != parents[0]
		|| source["tree"] != tree
		|| source["overlay"] != consumed
		|| receipt["session"] != session
		|| preview["subject"] != body.lines().next().unwrap_or("")
		|| actual.len() != 4
		|| preview["author"]["name"] != actual[0]
		|| preview["author"]["email"] != actual[1]
		|| preview["author"]["committer"]["name"] != actual[2]
		|| preview["author"]["committer"]["email"] != actual[3]
	{
		return Err("verification receipt cannot authorize this integrated commit".into());
	}
	validate_record(root, &policy, session, source, revision, false)?;
	Ok(())
}
