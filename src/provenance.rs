//! Bounded provider-reported metadata; no identity or ownership authority.
use crate::change_record::{
	self as record, Actor, Evidence, Operation, OverlayCommit, Provider, Storage, Target, Version,
};
use crate::{store::Store, Result};
use serde_json::{json, Value};
use std::{io::Read, path::Path};

#[derive(Default)]
pub(crate) struct Capture {
	pub records: Vec<Evidence>,
	pub omitted: u64,
	pub unknown: u64,
	actor: Option<Actor>,
}
impl Capture {
	pub fn check_session(&self, session: &str) -> Result<()> {
		if self.actor.as_ref().is_some_and(|a| a.session != session) {
			Err("capture actor/session mismatch".into())
		} else {
			Ok(())
		}
	}
	pub fn from_context(context: &Value) -> Result<Self> {
		let session = context["session"]
			.as_str()
			.filter(|s| !s.is_empty() && s.len() <= 256 && !s.contains('\0'))
			.ok_or("invalid attribution session")?;
		if !valid_session(session) {
			return Err("invalid attribution session reference".into());
		}
		let optional = |key: &str| -> Result<Option<String>> {
			match context.get(key) {
				None | Some(Value::Null) => Ok(None),
				Some(Value::String(s)) if !s.is_empty() && s.len() <= 256 && !s.contains('\0') => {
					Ok(Some(s.clone()))
				}
				_ => Err(format!("invalid attribution {key}")),
			}
		};
		Ok(Self {
			actor: Some(Actor {
				session: session.into(),
				run: optional("run")?,
				call: optional("call")?,
			}),
			..Self::default()
		})
	}
	pub fn reserve(
		&self,
		store: &Store,
		rel: &Path,
		op: Operation,
		storage: Storage,
	) -> Result<Option<Evidence>> {
		let Some(actor) = &self.actor else {
			return Ok(None);
		};
		let target = store
			.root()
			.join(rel)
			.components()
			.collect::<std::path::PathBuf>();
		let e = Evidence {
			v: 1,
			publication_id: record::publication_id()?,
			provider: Provider::Gitfs,
			operation: op,
			actor: actor.clone(),
			target: Target {
				workspace: store.root().to_str().ok_or("non-UTF8 repository")?.into(),
				path: target.to_str().ok_or("non-UTF8 target")?.into(),
				storage,
				overlay_ref: Some(store.branch(&actor.session)),
			},
			before: Version::Absent {},
			after: Version::Absent {},
			overlay_commit: OverlayCommit {
				before: None,
				after: Some("0".repeat(40)),
			},
		};
		e.validate(&actor.session)?;
		Ok(Some(e))
	}
	pub fn keep(&mut self, evidence: Option<Evidence>) {
		let Some(e) = evidence else { return };
		if self.records.len() >= record::batch() {
			self.omitted = self.omitted.saturating_add(1);
			return;
		}
		self.records.push(e);
		let args = self.args();
		if serde_json::to_vec(&args).map_or(true, |b| b.len() > record::batch_bytes()) {
			self.records.pop();
			self.omitted = self.omitted.saturating_add(1);
		}
	}
	pub fn uncertain(&mut self) {
		if self.actor.is_some() {
			self.unknown = self.unknown.saturating_add(1);
		}
	}
	fn args(&self) -> Value {
		json!({"op":"record_changes","id":self.actor.as_ref().map(|a|&a.session),"records":self.records})
	}
}
/// A session id that names a safe `refs/gitfs/<session>` reference.
pub(crate) fn valid_session(session: &str) -> bool {
	!session.is_empty()
		&& session.len() <= 256
		&& !session.contains("..")
		&& !session.contains("@{")
		&& !session
			.bytes()
			.any(|b| b <= b' ' || b == 127 || b"~^:?*[\\".contains(&b))
		&& !session
			.split('/')
			.any(|s| s.is_empty() || s.starts_with('.') || s.ends_with('.') || s.ends_with(".lock"))
}

pub(crate) fn preimage(path: &Path) -> Result<Version> {
	let cap = crate::limits::limits().provenance_bytes;
	let initial = match std::fs::symlink_metadata(path) {
		Ok(v) => v,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Version::Absent {}),
		Err(e) => return Err(e.to_string()),
	};
	if !initial.is_file() || initial.len() > cap {
		return Err("unsafe or oversized attribution preimage".into());
	}
	let mut options = std::fs::OpenOptions::new();
	options.read(true);
	// POSIX-only hardening: no blocking on a FIFO, no following a symlink.
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
	}
	let mut file = options.open(path).map_err(|e| e.to_string())?;
	let metadata = file.metadata().map_err(|e| e.to_string())?;
	if !metadata.is_file() || metadata.len() > cap {
		return Err("unsafe or oversized attribution preimage".into());
	}
	let mut bytes = Vec::new();
	(&mut file)
		.take(cap + 1)
		.read_to_end(&mut bytes)
		.map_err(|e| e.to_string())?;
	if bytes.len() as u64 > cap {
		return Err("attribution preimage grew beyond cap".into());
	}
	Ok(content_version(&bytes))
}
pub(crate) fn content_version(bytes: &[u8]) -> Version {
	use sha2::{Digest, Sha256};
	Version::Sha256 {
		value: format!("{:x}", Sha256::digest(bytes)),
	}
}

/// Where mutation evidence would be recorded. git-fs has no `sessions`
/// service to record to, so the recorder only shapes the answer: the
/// capture's counts travel in `attribution` and nothing is published.
#[derive(Clone)]
pub(crate) enum Recorder {
	Absent,
}
impl Recorder {
	pub async fn finish(&self, outcome: Result<Value>, capture: Capture) -> Result<Value> {
		let mut result = outcome.unwrap_or_else(crate::tool_result::failure);
		let mut body = match result["content"].take() {
			object @ Value::Object(_) => object,
			other => serde_json::from_str::<Value>(other.as_str().unwrap_or_default())
				.unwrap_or_else(|_| json!({"message": other})),
		};
		if !body.is_object() {
			body = json!({"result":body});
		}
		let reason = (capture.omitted > 0 || capture.unknown > 0).then_some(
			"metadata capacity or unsafe preimage/uncertain mutation left paths unrecorded",
		);
		body["attribution"] = json!({"status":"unavailable","recorded_ids":[],"retained":capture.records.len(),"omitted":capture.omitted,"unknown":capture.unknown,"reason":reason,"trust":"provider_reported_not_authorization"});
		result["content"] = body;
		Ok(result)
	}
}
#[cfg(test)]
#[path = "../tests/unit/provenance.rs"]
mod tests;
