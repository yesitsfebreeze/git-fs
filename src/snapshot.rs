//! Run-owned checkouts of the exact reviewed integration candidate. A
//! candidate is not the shared live tip and never updates trunk or its index.

use super::{ReviewedTree, Store};
use crate::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "gitfs-candidate-snapshot/v1";

#[derive(Serialize, Deserialize)]
struct CandidateRecord {
	schema: String,
	id: String,
	owner: String,
	capability_hash: String,
	repository: String,
	path: String,
	commit: String,
	review: ReviewedTree,
}

struct TreeFile {
	path: String,
	mode: String,
	oid: String,
}

fn git_at(root: &Path, args: &[&str]) -> Result<String> {
	let root_arg = crate::hook::git_command_path(root)?;
	let output = Command::new("git")
		.arg("-C")
		.arg(&root_arg)
		.args(args)
		.output()
		.map_err(|e| format!("candidate git execution: {e}"))?;
	if !output.status.success() {
		return Err(format!(
			"candidate git {}: {}",
			args.join(" "),
			String::from_utf8_lossy(&output.stderr).trim()
		));
	}
	String::from_utf8(output.stdout)
		.map(|s| s.trim().to_owned())
		.map_err(|e| format!("candidate git output: {e}"))
}

fn owner_digest(session: &str) -> String {
	format!("{:x}", Sha256::digest(session.as_bytes()))
}

fn validated_id(id: &str, owner: &str) -> Result<()> {
	let prefix = format!("candidate-{}-", &owner[..12]);
	let suffix = id
		.strip_prefix(&prefix)
		.ok_or("candidate snapshot belongs to a different session")?;
	if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
		return Err("invalid candidate snapshot ID".into());
	}
	Ok(())
}

fn tree_files(store: &Store, tree: &str) -> Result<Vec<TreeFile>> {
	let raw = store.git(&["ls-tree", "-rz", tree])?;
	let mut entries = Vec::new();
	for raw_entry in raw
		.split(|byte| *byte == 0)
		.filter(|entry| !entry.is_empty())
	{
		let tab = raw_entry
			.iter()
			.position(|byte| *byte == b'\t')
			.ok_or("malformed candidate tree entry")?;
		let header =
			std::str::from_utf8(&raw_entry[..tab]).map_err(|_| "non-UTF-8 candidate tree entry")?;
		let mut fields = header.split_whitespace();
		let mode = fields.next().ok_or("missing candidate tree mode")?;
		let kind = fields.next().ok_or("missing candidate tree kind")?;
		let oid = fields.next().ok_or("missing candidate blob id")?;
		if fields.next().is_some() || kind != "blob" || !matches!(mode, "100644" | "100755") {
			return Err(format!("unsupported candidate tree entry: {header}; symlinks and submodules cannot be verified"));
		}
		if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Err("malformed candidate blob id".into());
		}
		let rel = std::str::from_utf8(&raw_entry[tab + 1..])
			.map_err(|_| "non-UTF-8 candidate checkout path is unsupported")?;
		if rel.is_empty()
			|| rel.chars().any(|c| matches!(c, '\n' | '\r' | '\\' | ':'))
			|| rel.starts_with('/')
			|| !Path::new(rel)
				.components()
				.all(|component| matches!(component, Component::Normal(_)))
		{
			return Err(format!("unsupported candidate checkout path: {rel:?}"));
		}
		entries.push(TreeFile {
			path: rel.to_owned(),
			mode: mode.to_owned(),
			oid: oid.to_owned(),
		});
	}
	Ok(entries)
}

fn verify_checkout(root: &Path, commit: &str, tree: &str, entries: &[TreeFile]) -> Result<()> {
	if git_at(root, &["rev-parse", "HEAD"])? != commit {
		return Err("candidate checkout HEAD differs from owned commit".into());
	}
	if git_at(root, &["rev-parse", "HEAD^{tree}"])? != tree {
		return Err("candidate checkout tree differs from integration review".into());
	}
	let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
	for file in entries {
		let path = root.join(&file.path);
		let metadata = fs::symlink_metadata(&path)
			.map_err(|e| format!("candidate file {}: {e}", file.path))?;
		if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
			return Err(format!(
				"candidate file {} is not a regular file",
				file.path
			));
		}
		if !path
			.canonicalize()
			.map_err(|e| e.to_string())?
			.starts_with(&canonical_root)
		{
			return Err(format!("candidate file {} escapes checkout", file.path));
		}
		let actual = git_at(root, &["hash-object", "--no-filters", "--", &file.path])?;
		if actual != file.oid {
			return Err(format!(
				"candidate file {} was transformed by checkout/EOL/filter",
				file.path
			));
		}
		let stage = git_at(root, &["ls-files", "--stage", "--", &file.path])?;
		let expected = format!("{} {} 0\t", file.mode, file.oid);
		if !stage.starts_with(&expected) || stage.lines().count() != 1 {
			return Err(format!(
				"candidate file {} has unexpected Git mode/index",
				file.path
			));
		}
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			let executable = metadata.permissions().mode() & 0o111 != 0;
			if executable != (file.mode == "100755") {
				return Err(format!(
					"candidate file {} has unexpected executable bit",
					file.path
				));
			}
		}
	}
	if !git_at(root, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
		return Err("candidate checkout has unexpected changes or untracked files".into());
	}
	Ok(())
}

impl Store {
	fn candidate_dir(&self, create: bool) -> Result<PathBuf> {
		let root = self.root.canonicalize().map_err(|e| e.to_string())?;
		let store = self.dir.canonicalize().map_err(|e| e.to_string())?;
		if store == root || !store.starts_with(&root) {
			return Err("candidate store escapes repository".into());
		}
		let snapshots = self.dir.join("snapshots");
		match fs::symlink_metadata(&snapshots) {
			Ok(metadata) if metadata.file_type().is_symlink() => {
				return Err("candidate snapshot directory is a symlink".into());
			}
			Ok(metadata) if !metadata.is_dir() => {
				return Err("candidate snapshot directory is not a directory".into());
			}
			Ok(_) => {}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound && create => {
				fs::create_dir(&snapshots).map_err(|e| e.to_string())?;
			}
			Err(error) => return Err(format!("candidate snapshot directory unavailable: {error}")),
		}
		let snapshots = snapshots.canonicalize().map_err(|e| e.to_string())?;
		if snapshots.parent() != Some(store.as_path()) || !snapshots.starts_with(root) {
			return Err("candidate snapshot directory escapes repository".into());
		}
		Ok(snapshots)
	}

	/// Export the exact tree that `integrate` would currently review, into an
	/// isolated Git worktree pinned to a synthetic detached commit.
	pub fn create_candidate_snapshot(&self, session: &str) -> Result<Value> {
		self.create_candidate_snapshot_checked(session, None, None)
	}

	/// Optionally bind creation to the source tree/review returned by an
	/// earlier integration preview. A changed HEAD/overlay/index fails closed.
	pub fn create_candidate_snapshot_checked(
		&self,
		session: &str,
		expected_tree: Option<&str>,
		expected_review: Option<&Value>,
	) -> Result<Value> {
		if self.git_dir.is_some() {
			return Err("candidate snapshots require a project Git repository".into());
		}
		if !crate::provenance::valid_session(session) {
			return Err("invalid candidate session".into());
		}
		let review = self.reviewed_tree(session)?;
		if expected_tree.is_some_and(|expected| expected != review.tree) {
			return Err("candidate tree changed since preview; review again".into());
		}
		if expected_review.is_some_and(|expected| *expected != json!(review)) {
			return Err("candidate review changed since preview; review again".into());
		}
		let entries = tree_files(self, &review.tree)?;
		let commit = self
			.git_str(&[
				"-c",
				"user.name=git-fs",
				"-c",
				"user.email=git-fs@invalid.example",
				"commit-tree",
				&review.tree,
				"-p",
				&review.head,
			])?
			.trim()
			.to_owned();
		let snapshots = self.candidate_dir(true)?;
		let repository = self
			.root
			.canonicalize()
			.map_err(|e| e.to_string())?
			.to_string_lossy()
			.replace('\\', "/");
		let owner = owner_digest(session);
		let mut capability_bytes = [0_u8; 32];
		getrandom::fill(&mut capability_bytes)
			.map_err(|e| format!("candidate capability generation: {e}"))?;
		let capability = capability_bytes
			.iter()
			.map(|b| format!("{b:02x}"))
			.collect::<String>();
		let capability_hash = owner_digest(&capability);
		let nonce = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map_err(|e| e.to_string())?
			.as_nanos();
		let id = format!(
			"candidate-{}-{}-{:x}-{nonce:x}",
			&owner[..12],
			&review.tree[..12],
			std::process::id()
		);
		let checkout = snapshots.join(&id);
		let checkout_arg = crate::hook::git_command_path(&checkout)?;
		let record_path = snapshots.join(format!("{id}.json"));
		if checkout.exists() {
			return Err("candidate snapshot ID collision".into());
		}
		let record = CandidateRecord {
			schema: SCHEMA.to_owned(),
			id: id.clone(),
			owner,
			capability_hash,
			repository,
			path: checkout_arg.clone(),
			commit: commit.clone(),
			review: review.clone(),
		};
		let mut file = OpenOptions::new()
			.create_new(true)
			.write(true)
			.open(&record_path)
			.map_err(|e| format!("candidate manifest reservation: {e}"))?;
		serde_json::to_writer_pretty(&mut file, &record).map_err(|e| e.to_string())?;
		file.write_all(b"\n").map_err(|e| e.to_string())?;
		file.sync_all().map_err(|e| e.to_string())?;
		drop(file);

		let mut added = false;
		let outcome = (|| -> Result<()> {
			self.git(&["worktree", "add", "--detach", "--", &checkout_arg, &commit])?;
			added = true;
			verify_checkout(&checkout, &commit, &review.tree, &entries)
		})();
		if let Err(error) = outcome {
			if added {
				if let Err(cleanup) = self.git(&["worktree", "remove", "--", &checkout_arg]) {
					return Err(format!("candidate verification refused: {error}; owned checkout retained at {checkout_arg}: {cleanup}"));
				}
			} else if checkout.exists() {
				return Err(format!("candidate creation failed: {error}; partial checkout retained at {checkout_arg}"));
			}
			fs::remove_file(&record_path)
				.map_err(|e| format!("candidate failed: {error}; manifest cleanup: {e}"))?;
			return Err(error);
		}

		Ok(json!({
			"schema": SCHEMA,
			"kind": "candidate",
			"id": id,
			"capability": capability,
			"path": checkout_arg,
			"revision": commit,
			"tree": review.tree,
			"source": review,
			"manifest": record_path.to_string_lossy().replace('\\', "/"),
			"isolation": "run-owned detached worktree; native tools may access external files, services and network"
		}))
	}

	/// Release only a matching capability-bound, still-clean snapshot; never use force.
	/// Distinct runs of the same session cannot release each other's snapshots.
	pub fn release_candidate_snapshot(
		&self,
		session: &str,
		id: &str,
		capability: &str,
	) -> Result<Value> {
		if self.git_dir.is_some() {
			return Err("candidate snapshots require a project Git repository".into());
		}
		let owner = owner_digest(session);
		validated_id(id, &owner)?;
		let snapshots = self.candidate_dir(false)?;
		let manifest_path = snapshots.join(format!("{id}.json"));
		let record: CandidateRecord = serde_json::from_slice(
			&fs::read(&manifest_path).map_err(|e| format!("candidate manifest missing: {e}"))?,
		)
		.map_err(|e| format!("candidate manifest invalid: {e}"))?;
		let repository = self
			.root
			.canonicalize()
			.map_err(|e| e.to_string())?
			.to_string_lossy()
			.replace('\\', "/");
		let checkout = snapshots.join(id);
		let checkout_arg = crate::hook::git_command_path(&checkout)?;
		if capability.len() != 64 || !capability.bytes().all(|b| b.is_ascii_hexdigit()) {
			return Err("candidate release capability required".into());
		}
		if record.schema != SCHEMA
			|| record.id != id
			|| record.owner != owner
			|| record.capability_hash != owner_digest(capability)
			|| record.repository != repository
			|| record.path != checkout_arg
		{
			return Err("candidate manifest ownership or repository mismatch".into());
		}
		let actual = checkout
			.canonicalize()
			.map_err(|e| format!("candidate checkout unavailable (manifest retained): {e}"))?;
		if actual.parent() != Some(snapshots.as_path()) || actual != checkout {
			return Err("candidate checkout escapes owned snapshot directory".into());
		}
		let git_top = git_at(&actual, &["rev-parse", "--show-toplevel"])?;
		let git_top = Path::new(&git_top)
			.canonicalize()
			.map_err(|e| e.to_string())?;
		if git_top != actual
			|| git_at(&actual, &["rev-parse", "HEAD"])? != record.commit
			|| git_at(&actual, &["rev-parse", "HEAD^{tree}"])? != record.review.tree
		{
			return Err("candidate checkout identity changed; refusing cleanup".into());
		}
		if !git_at(
			&actual,
			&[
				"status",
				"--porcelain",
				"--untracked-files=all",
				"--ignored=matching",
			],
		)?
		.is_empty()
		{
			return Err(
				"candidate contains mutable or ignored files; refusing cleanup of foreign outputs"
					.into(),
			);
		}
		self.git(&["worktree", "remove", "--", &checkout_arg])?;
		fs::remove_file(&manifest_path)
			.map_err(|e| format!("candidate checkout released but manifest cleanup failed: {e}"))?;
		Ok(json!({"released":id,"kind":"candidate","tree":record.review.tree}))
	}
}
