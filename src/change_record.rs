//! The change record the sessions kern accepts (`record_changes`): a copy
//! of its contract, kept in step by hand. Reported publication coordinates are
//! evidence, never authorization or ownership.
#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

// The caps the sessions kern declares (`limits.evidence_bytes`,
// `limits.batch_bytes`, `limits.batch`); a record it would refuse is not sent.
pub fn evidence_bytes() -> usize {
	1_048_576
}
pub fn batch_bytes() -> usize {
	16_777_216
}
pub fn batch() -> usize {
	1024
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
	Fs,
	Gitfs,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
	Write,
	Edit,
	Capture,
	Materialize,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Storage {
	Direct,
	GitfsOverlay,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Actor {
	pub session: String,
	#[serde(deserialize_with = "Option::<String>::deserialize")]
	pub run: Option<String>,
	#[serde(deserialize_with = "Option::<String>::deserialize")]
	pub call: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Target {
	pub workspace: String,
	pub path: String,
	pub storage: Storage,
	#[serde(deserialize_with = "Option::<String>::deserialize")]
	pub overlay_ref: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Version {
	Absent {},
	Sha256 { value: String },
	GitObject { value: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OverlayCommit {
	#[serde(deserialize_with = "Option::<String>::deserialize")]
	pub before: Option<String>,
	#[serde(deserialize_with = "Option::<String>::deserialize")]
	pub after: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
	pub v: u8,
	pub publication_id: String,
	pub provider: Provider,
	pub operation: Operation,
	pub actor: Actor,
	pub target: Target,
	pub before: Version,
	pub after: Version,
	pub overlay_commit: OverlayCommit,
}

pub fn publication_id() -> Result<String, String> {
	let mut bytes = [0u8; 32];
	getrandom::fill(&mut bytes).map_err(|e| format!("publication identity unavailable: {e}"))?;
	Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}
pub fn digest<T: Serialize>(value: &T) -> Result<String, String> {
	Ok(format!(
		"{:x}",
		Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
	))
}
pub fn hex(value: &str, sizes: &[usize]) -> bool {
	sizes.contains(&value.len())
		&& value
			.bytes()
			.all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn bounded(value: &str, cap: usize) -> bool {
	!value.is_empty() && value.len() <= cap && !value.contains('\0')
}
fn normalized(value: &str) -> bool {
	let path = Path::new(value);
	// Compared as paths, component-wise: git prints `/` on Windows, native
	// separators are `\`, and an os_str compare would reject every workspace.
	let rebuilt: std::path::PathBuf = path.components().collect();
	path.is_absolute()
		&& !path
			.components()
			.any(|c| matches!(c, Component::CurDir | Component::ParentDir))
		&& rebuilt.as_path() == path
}
impl Evidence {
	pub fn validate(&self, session: &str) -> Result<(), String> {
		if self.v != 1 || !hex(&self.publication_id, &[64]) {
			return Err("invalid evidence schema or publication_id".into());
		}
		if self.actor.session != session
			|| !bounded(session, 256)
			|| [&self.actor.run, &self.actor.call]
				.into_iter()
				.flatten()
				.any(|v| !bounded(v, 256))
		{
			return Err("invalid evidence actor/session".into());
		}
		if !bounded(&self.target.workspace, 4096)
			|| !bounded(&self.target.path, 4096)
			|| !normalized(&self.target.workspace)
			|| !normalized(&self.target.path)
			|| !Path::new(&self.target.path).starts_with(&self.target.workspace)
			|| self.target.path == self.target.workspace
		{
			return Err("invalid evidence target".into());
		}
		let overlay = self.target.storage == Storage::GitfsOverlay;
		match (&self.provider, &self.operation) {
			(Provider::Fs, Operation::Write | Operation::Edit) if !overlay => (),
			(Provider::Gitfs, Operation::Materialize) if !overlay => (),
			(Provider::Gitfs, Operation::Write | Operation::Edit | Operation::Capture)
				if overlay => {}
			_ => return Err("invalid provider/operation/storage".into()),
		}
		if self.provider == Provider::Fs {
			if self.target.overlay_ref.is_some()
				|| self.overlay_commit.before.is_some()
				|| self.overlay_commit.after.is_some()
			{
				return Err("FS has no overlay coordinates".into());
			}
		} else {
			let r = self
				.target
				.overlay_ref
				.as_deref()
				.ok_or("GitFS overlay_ref required")?;
			if !bounded(r, 1024)
				|| !r.starts_with("refs/gitfs/")
				|| r.len() == 11
				|| r.contains("..")
				|| r.contains("@{")
				|| r.bytes()
					.any(|b| b <= b' ' || b == 127 || b"~^:?*[\\".contains(&b))
				|| r.split('/').any(|p| {
					p.is_empty() || p.starts_with('.') || p.ends_with('.') || p.ends_with(".lock")
				}) {
				return Err("invalid overlay_ref".into());
			}
			if self.overlay_commit.after.is_none() {
				return Err("GitFS final overlay commit required".into());
			}
		}
		for value in [&self.overlay_commit.before, &self.overlay_commit.after]
			.into_iter()
			.flatten()
		{
			if !hex(value, &[40, 64]) {
				return Err("invalid overlay commit".into());
			}
		}
		for version in [&self.before, &self.after] {
			match version {
				Version::Absent {} => (),
				Version::Sha256 { value } if !overlay && hex(value, &[64]) => (),
				Version::GitObject { value } if overlay && hex(value, &[40, 64]) => (),
				_ => return Err("invalid revision kind or digest".into()),
			}
		}
		if serde_json::to_vec(self).map_err(|e| e.to_string())?.len() > evidence_bytes() {
			return Err("evidence exceeds 16 KiB".into());
		}
		Ok(())
	}
}
