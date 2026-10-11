//! What remains of the kern host bridge: the review gate `integrate` may ask
//! before it commits. The gate is an OpenAI-compatible chat endpoint named
//! by `GIT_FS_GATE_URL`, `GIT_FS_GATE_KEY` and `GIT_FS_GATE_MODEL`; with no
//! URL set there is no gate, and `sessions` attribution is never recorded.

use serde_json::{json, Value};

#[derive(Clone, Copy, Debug)]
pub struct Ctx;

impl Ctx {
	/// The only calls the kern code still makes: `router session` for the gate
	/// endpoint and `router release` for its lease. Anything else is absent.
	pub async fn call(&self, kern: &str, args: Value) -> Result<Value, String> {
		match (kern, args["op"].as_str()) {
			("router", Some("session")) => {
				let base = std::env::var("GIT_FS_GATE_URL")
					.ok()
					.filter(|s| !s.is_empty())
					.ok_or("no review gate configured (GIT_FS_GATE_URL unset)")?;
				let key = std::env::var("GIT_FS_GATE_KEY").unwrap_or_default();
				Ok(json!({"base": base, "key": key}))
			}
			("router", Some("release")) => Ok(Value::Null),
			(kern, _) => Err(format!("`{kern}` is not reachable from git-fs")),
		}
	}
}
