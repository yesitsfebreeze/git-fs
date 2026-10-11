//! The caps this kern spends, in one place and settable once.
//!
//! They were `const`s beside the code that spent them. They are config:
//! declared in `kern.md` under `limits`, [`install`]ed from the
//! host's configuration before anything is served, and read here by the
//! modules that spend them.
//!
//! The numbers live in `kern.md` and nowhere else. This struct names the
//! keys to give them types; [`Limits::declared`] reads the defaults back off
//! that same document for a run with no host to settle them.

use std::sync::OnceLock;

/// Every cap the Git surface spends. Generous: a real repository's history and
/// a real push's output are both larger than a cautious guess at them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
	/// Bytes of provenance read from one source.
	pub provenance_bytes: u64,
	/// Bytes of one source file inspected.
	pub source_bytes: u64,
	/// Push records kept, fields on one, and bytes of push output carried.
	pub push_records: usize,
	pub push_record_bytes: usize,
	pub push_output_bytes: usize,
}

impl Limits {
	/// What this kern's own document declares, for a run with no host to
	/// settle it — a unit test, or a standalone invocation.
	pub fn declared() -> Self {
		let defaults = crate::config::declared(crate::document());
		serde_json::from_value(defaults["limits"].clone())
			.expect("declared limits match their keys")
	}
}

static LIMITS: OnceLock<Limits> = OnceLock::new();

/// Set the caps for this process. The first call wins, so a push already
/// under way keeps the budget it started with. Answers whether this call set
/// them.
pub fn install(limits: Limits) -> bool {
	LIMITS.set(limits).is_ok()
}

/// The caps in force: what was installed, or what the document declares.
pub fn limits() -> &'static Limits {
	LIMITS.get_or_init(Limits::declared)
}
