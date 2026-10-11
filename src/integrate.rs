//! Reviewed local publication; remote actions are explicitly separate.

use crate::bridge::Ctx;
use crate::secrets;
use crate::store::{Owned, Store, StoreConfig};
use crate::tool_result::success as ok;
use crate::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Config {
	pub store: Arc<StoreConfig>,
	pub push: bool,
	pub remote: String,
	pub gate_model: Option<String>,
	pub gate_timeout_ms: usize,
	pub push_timeout_ms: u64,
	pub force_secrets: bool,
	pub author_name: String,
	pub author_email: String,
	configuration_valid: bool,
	active: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl Config {
	pub fn from(cfg: &Value, store: Arc<StoreConfig>) -> Self {
		Self {
			configuration_valid: valid_config(cfg),
			push_timeout_ms: cfg["push_timeout_ms"].as_u64().unwrap_or(60_000),
			store,
			push: cfg["push"].as_bool().unwrap_or(true),
			remote: cfg["remote"]
				.as_str()
				.map(str::to_owned)
				.unwrap_or_else(|| "origin".into()),
			gate_model: cfg["gate_model"].as_str().map(str::to_owned),
			gate_timeout_ms: cfg["gate_timeout_ms"]
				.as_u64()
				.map(|v| v as usize)
				.unwrap_or(300_000),
			force_secrets: cfg["force_secrets"].as_bool().unwrap_or(false),
			author_name: cfg["author"]["name"]
				.as_str()
				.unwrap_or("kern gitfs")
				.into(),
			author_email: cfg["author"]["email"]
				.as_str()
				.unwrap_or("gitfs@kern.local")
				.into(),
			active: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

fn valid_config(cfg: &Value) -> bool {
	if cfg.is_null() {
		return true;
	}
	if !cfg.is_object() {
		return false;
	}
	// The host settles an optional key with no value as `null`; that is absent, not wrong.
	let get = |key: &str| cfg.get(key).filter(|v| !v.is_null());
	for key in ["push", "force_secrets"] {
		if get(key).is_some_and(|v| !v.is_boolean()) {
			return false;
		}
	}
	for key in ["remote", "gate_model"] {
		if get(key).is_some_and(|v| !v.is_string()) {
			return false;
		}
	}
	// Bounds as kern.md declares them.
	for (key, bounds) in [
		("gate_timeout_ms", 1..=86_400_000),
		("push_timeout_ms", 100..=60_000),
	] {
		if get(key).is_some_and(|v| !v.as_u64().is_some_and(|n| bounds.contains(&n))) {
			return false;
		}
	}
	if let Some(author) = get("author") {
		if !author.is_object() || !author["name"].is_string() || !author["email"].is_string() {
			return false;
		}
	}
	true
}

#[cfg(test)]
mod valid_config_tests {
	use super::valid_config;
	use serde_json::json;

	#[test]
	fn settled_defaults_are_valid() {
		// Exactly what the host hands over when nothing overrides the declaration.
		let config = json!({
			"push": true, "remote": "origin", "gate_model": null,
			"gate_timeout_ms": 300000, "push_timeout_ms": 60000, "force_secrets": false,
			"author": {"name": "kern gitfs", "email": "gitfs@kern.local"}
		});
		assert!(valid_config(&config));
		assert!(!valid_config(&json!({"gate_model": 3})));
		assert!(!valid_config(&json!({"push_timeout_ms": 0})));
		// push.rs refuses a push timeout above 60 s.
		assert!(!valid_config(&json!({"push_timeout_ms": 300000})));
		let default = super::Config::from(
			&json!({}),
			std::sync::Arc::new(crate::store::StoreConfig::new(".kern")),
		)
		.push_timeout_ms;
		assert!((100..=60_000).contains(&default));
	}
}

pub const INTEGRATE_TRAILER: &str = "Integrated-by: gitfs";

/// The terminal trailer block a integrate commit carries; `push` accepts exactly this.
pub(crate) fn receipt(session: &str, revision: &str, consumed: &str) -> String {
	format!("{INTEGRATE_TRAILER}\nGitfs-Session: {session}\nGitfs-Review: {revision}\nGitfs-Consumed: {consumed}\n")
}

pub(crate) fn describe() -> Value {
	json!({
		"name":"integrate",
		"description":"Review owned changes before publication: integrate preview then commit publish locally, push publishes to a remote, and there is never an implicit push. Every commit needs the revision its preview returned. Full guidance: `kern help fs`.",
		"input_schema":{
			"type":"object",
			"required":["op"],
			"properties":{
				"op":{"type":"string","enum":["integrate","push","undo","scan"]},
				"message":{"type":"string","description":"Reviewed commit subject; never bypasses a required gate."},
				"phase":{"type":"string","enum":["preview","commit","reconcile"]},
				"expected_revision":{"type":"string"},
				"remote":{"type":"string"},"ref":{"type":"string"},"commit":{"type":"string"},
				"expected_remote_head":{"type":["string","null"]},"operation_id":{"type":"string"},
				"force_secrets":{"type":"boolean","description":"Integrate despite secrets in the diff."},
				"paths":{"type":"array","items":{"type":"string"},"description":"Files to scan (scan)."}
			},
			"additionalProperties":false
		}
	})
}

fn err(content: impl Into<String>) -> Result<Value> {
	Ok(json!({"content": content.into(), "error": true}))
}

fn session_of(args: &Value) -> Result<String> {
	args["context"]["session"]
		.as_str()
		.filter(|s| {
			!s.is_empty()
				&& s.len() <= 128
				&& s.bytes()
					.all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
		})
		.map(str::to_owned)
		.ok_or("session required".into())
}

pub async fn handle(host: Ctx, cfg: &Config, args: Value) -> Result<Value> {
	match args["op"].as_str() {
		Some("describe") => Ok(describe()),
		Some("cancel") => {
			let key = invocation_key(&args)?;
			let active = cfg.active.lock().unwrap_or_else(|e| e.into_inner());
			let found = active.get(&key);
			if let Some(cancel) = found {
				cancel.store(true, Ordering::SeqCst);
			}
			ok(
				json!({"cancellation_requested":found.is_some(),"completed_effects_not_rolled_back":true}),
			)
		}
		Some("call") => {
			crate::tool_result::validate("integrate", &args["input"])?;
			let session = session_of(&args)?;
			let cwd = args["context"]["cwd"]
				.as_str()
				.map(std::path::PathBuf::from)
				.ok_or_else(|| "cwd required".to_string())?;
			if args["input"]["op"].as_str() == Some("push") {
				if !cfg.configuration_valid {
					return Err("invalid integrate configuration".into());
				}
				let guard = Active::start(cfg, invocation_key(&args)?)?;
				let cancel = guard.cancel.clone();
				let input = args["input"].clone();
				let config = cfg.store.clone();
				let timeout_ms = cfg.push_timeout_ms;
				let deadline =
					std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
				let result = tokio::task::spawn_blocking(move || {
					crate::push::execute(
						&config, &cwd, &session, &input, timeout_ms, deadline, &cancel,
					)
				})
				.await
				.map_err(|_| "push outcome unknown; reconcile recorded attempt")??;
				return ok(result);
			}
			let store = cfg.store.at(&cwd)?;
			let input = args["input"].clone();
			match input["op"].as_str() {
				Some("undo") | Some("scan") => {
					let cfg = cfg.clone();
					tokio::task::spawn_blocking(move || dispatch(&cfg, &store, &session, &input))
						.await
						.map_err(|e| format!("integrate task failed: {e}"))?
				}
				Some("integrate") => {
					let guard = Active::start(cfg, invocation_key(&args)?)?;
					integrate(host, cfg, &store, &session, &input, guard.cancel.clone()).await
				}
				Some(other) => err(format!("unknown integrate op: {other}")),
				None => err("integrate op required"),
			}
		}
		Some(other) => err(format!("unknown integrate op: {other}")),
		None => err("integrate op required"),
	}
}

fn dispatch(cfg: &Config, store: &Store, session: &str, input: &Value) -> Result<Value> {
	match input["op"].as_str() {
		Some("undo") => {
			let branch = store
				.branch_name()?
				.ok_or("detached HEAD; undo not supported")?;
			let parent = store.undo(&cfg.remote, &branch)?;
			ok(json!({"undone": true, "branch": branch, "parent": parent}))
		}
		Some("scan") => {
			let findings = match input["paths"].as_array() {
				Some(list) => {
					let mut out = Vec::new();
					for path in list.iter().filter_map(Value::as_str) {
						let abs = store.root().join(path);
						if let Ok(text) = std::fs::read_to_string(&abs) {
							let name = std::path::Path::new(path)
								.file_name()
								.map(|b| b.to_string_lossy().into_owned())
								.unwrap_or_default();
							out.extend(secrets::scan_file(path, &name, &text));
						}
					}
					out
				}
				None => {
					let _ = session;
					let (tree, _owned) = store.preview_integrate(session)?;
					let diff = store.tree_diff(&tree)?;
					secrets::scan_diff(&diff)
				}
			};
			ok(json!({
				"findings": findings.iter().map(|f| json!({"file": f.file, "issue": f.issue})).collect::<Vec<_>>()
			}))
		}
		Some(other) => err(format!("unknown integrate op: {other}")),
		None => err("integrate op required"),
	}
}

fn invocation_key(args: &Value) -> Result<String> {
	session_of(args)?;
	let context = &args["context"];
	for key in ["session", "run", "call", "cwd"] {
		if !context[key]
			.as_str()
			.is_some_and(|s| !s.is_empty() && s.len() <= 4096 && !s.contains('\0'))
		{
			return Err("bounded trusted session/run/call/cwd required".into());
		}
	}
	Ok(json!([
		context["session"],
		context["run"],
		context["call"],
		context["cwd"]
	])
	.to_string())
}
struct Active {
	active: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
	key: String,
	cancel: Arc<AtomicBool>,
}
impl Active {
	fn start(cfg: &Config, key: String) -> Result<Self> {
		let mut active = cfg.active.lock().unwrap_or_else(|e| e.into_inner());
		if active.len() >= 128 || active.contains_key(&key) {
			return Err("integrate invocation busy or capacity exceeded".into());
		}
		let cancel = Arc::new(AtomicBool::new(false));
		active.insert(key.clone(), cancel.clone());
		Ok(Self {
			active: cfg.active.clone(),
			key,
			cancel,
		})
	}
}
impl Drop for Active {
	fn drop(&mut self) {
		self.cancel.store(true, Ordering::SeqCst);
		self.active
			.lock()
			.unwrap_or_else(|e| e.into_inner())
			.remove(&self.key);
	}
}

fn identity(cfg: &Config) -> Result<Value> {
	if !cfg.configuration_valid {
		return Err("invalid integrate configuration".into());
	}
	for value in [&cfg.author_name, &cfg.author_email] {
		if value.trim().is_empty()
			|| value.len() > 256
			|| value.chars().any(|c| c.is_control() || "<>".contains(c))
		{
			return Err("invalid configured author identity".into());
		}
	}
	if !cfg.author_email.contains('@')
		|| !(1..=86_400_000).contains(&cfg.gate_timeout_ms)
		|| cfg
			.gate_model
			.as_ref()
			.is_some_and(|s| s.is_empty() || s.len() > 256 || s.contains('\0'))
	{
		return Err("invalid configured integrate gate or author".into());
	}
	Ok(
		json!({"name":cfg.author_name,"email":cfg.author_email,"committer":{"name":"kern gitfs","email":"gitfs@kern.local"}}),
	)
}

fn preview_tree(
	cfg: &Config,
	review: &crate::store::ReviewedTree,
	session: &str,
	subject: &str,
	force: bool,
) -> Result<Value> {
	Ok(
		json!({"schema":"gitfs-reviewed-commit/v1","source":review,"session":session,"subject":subject,"author":identity(cfg)?,"force_secrets":force,"required_gate":cfg.gate_model,"gate_timeout_ms":cfg.gate_timeout_ms}),
	)
}

async fn integrate(
	host: Ctx,
	cfg: &Config,
	store: &Store,
	session: &str,
	input: &Value,
	cancel: Arc<AtomicBool>,
) -> Result<Value> {
	let phase = input["phase"].as_str().unwrap_or("preview");
	if !["preview", "commit", "reconcile"].contains(&phase) {
		return err("unknown integrate phase");
	}
	let author = identity(cfg)?;
	// A replayed commit answers the receipt git already holds, and reconcile
	// asks for nothing else: whether the reviewed revision landed. This runs
	// before the tree is reviewed again: after a integrate the session owns
	// nothing, so a fresh review would refuse a replay that has in fact
	// already landed.
	if phase != "preview" {
		let expected = input["expected_revision"].as_str();
		if phase == "reconcile" && expected.is_none() {
			return err("reconcile needs the expected_revision its preview returned");
		}
		if let Some(expected) = expected {
			let found = {
				let store = store.clone();
				let expected = expected.to_owned();
				tokio::task::spawn_blocking(move || store.find_integrated(&expected))
					.await
					.map_err(|e| e.to_string())??
			};
			if let Some(found) = found {
				let mut receipt = json!({"phase":phase,"session":found["session"],"committed":found["committed"],"consumed":found["consumed"],"tree":found["tree"],"parent":found["parent"],"branch":store.branch_name()?.unwrap_or_default(),"subject":found["subject"],"author":author,"review_revision":expected,"push":false});
				if phase == "reconcile" {
					receipt["integrated"] = json!(true);
				}
				return ok(receipt);
			}
			if phase == "reconcile" {
				return ok(
					json!({"phase":"reconcile","integrated":false,"review_revision":expected,"push":false}),
				);
			}
		}
	}
	let (review, diff) = {
		let store = store.clone();
		let session = session.to_owned();
		tokio::task::spawn_blocking(move || -> Result<_> {
			let review = store.reviewed_tree(&session)?;
			let diff = store.reviewed_diff(&review)?;
			Ok((review, diff))
		})
		.await
		.map_err(|e| e.to_string())??
	};
	let subject = input["message"]
		.as_str()
		.map(str::to_owned)
		.unwrap_or_else(|| auto_subject(&review.owned));
	if subject.trim().is_empty()
		|| subject.len() > 1024
		|| subject.chars().any(char::is_control)
		|| has_secret_subject(&subject)
	{
		return err("invalid or secret-bearing commit subject");
	}
	let force = input["force_secrets"].as_bool().unwrap_or(false) || cfg.force_secrets;
	let tree = preview_tree(cfg, &review, session, &subject, force)?;
	let revision = format!(
		"{:x}",
		Sha256::digest(serde_json::to_vec(&tree).map_err(|e| e.to_string())?)
	);
	let findings = secrets::scan_diff(&diff);
	if phase == "preview" {
		if input.get("expected_revision").is_some() {
			return err("preview accepts no expected_revision; to integrate, send it with phase: \"commit\" and the same message");
		}
		return ok(
			json!({"phase":"preview","revision":revision,"tree":tree,"secrets_found":findings.len(),"push":false,"automatic_push_disabled":cfg.push}),
		);
	}
	if input["expected_revision"].as_str() != Some(&revision) {
		return err("reviewed inputs changed; preview again");
	}
	if cancel.load(Ordering::SeqCst) {
		return err("cancelled before publication");
	}
	if !findings.is_empty() && !force {
		return err("secrets found; no commit published");
	}
	if cfg.gate_model.is_some() {
		if diff.len() > 32 * 1024 {
			return err("required gate cannot review the complete diff within its limit");
		}
		tokio::time::timeout(
			std::time::Duration::from_millis(cfg.gate_timeout_ms as u64),
			gate(&host, cfg, &diff),
		)
		.await
		.map_err(|_| "required gate timed out; no commit published".to_string())??;
	}
	let store = store.clone();
	let result_session = session.to_owned();
	let session = session.to_owned();
	let name = cfg.author_name.clone();
	let email = cfg.author_email.clone();
	let parent = review.head.clone();
	let tree = review.tree.clone();
	let branch = review.branch.clone();
	let consumed = review.overlay.clone();
	let message = format!("{subject}\n\n{}", receipt(&session, &revision, &consumed));
	let (commit, worktree) = tokio::task::spawn_blocking(move || -> Result<_> {
		let commit = store.commit_reviewed(&session, &review, &message, &name, &email, &cancel)?;
		// The commit is on the ref; a sync that fails leaves the paths in `kept`.
		let worktree = store
			.sync_integrated(&review.head, &commit, &review.owned)
			.unwrap_or_else(|e| json!({"synced":[],"kept":[],"error":e}));
		Ok((commit, worktree))
	})
	.await
	.map_err(|e| format!("publication outcome unknown; reconcile before retry: {e}"))??;
	ok(
		json!({"phase":"commit","session":result_session,"committed":commit,"consumed":consumed,"tree":tree,"parent":parent,"branch":branch,"subject":subject,"author":author,"review_revision":revision,"push":false,"worktree":worktree}),
	)
}

fn has_secret_subject(subject: &str) -> bool {
	secrets::has_secret(subject)
}

fn auto_subject(owned: &[Owned]) -> String {
	if owned.is_empty() {
		"gitfs: update".into()
	} else if owned.len() == 1 {
		format!("gitfs: {}", owned[0].1)
	} else {
		format!("gitfs: update {} files", owned.len())
	}
}

struct Lease {
	host: Ctx,
	key: String,
}
impl Drop for Lease {
	fn drop(&mut self) {
		let host = self.host;
		let key = self.key.clone();
		tokio::spawn(async move {
			let _ = tokio::time::timeout(
				std::time::Duration::from_secs(1),
				host.call("router", json!({"op":"release","key":key})),
			)
			.await;
		});
	}
}

/// A configured model is a required gate, including explicit-message requests.
/// The caller bounds the complete lease/request/body/parse exchange.
async fn gate(host: &Ctx, cfg: &Config, diff: &str) -> Result<()> {
	let session = host.call("router", json!({"op":"session"})).await?;
	let base = session["base"].as_str().ok_or("router base required")?;
	let key = session["key"]
		.as_str()
		.filter(|s| !s.is_empty() && !s.contains('\0'))
		.ok_or("router session key required")?;
	let _lease = Lease {
		host: *host,
		key: key.into(),
	};
	let body = json!({"model":cfg.gate_model,"stream":false,"messages":[{"role":"user","content":gate_prompt(diff)}],"temperature":0});
	let mut response = reqwest::Client::new()
		.post(format!(
			"{}/v1/chat/completions",
			base.trim_end_matches('/')
		))
		.bearer_auth(key)
		.json(&body)
		.send()
		.await
		.map_err(|_| "required gate transport unavailable")?;
	if !response.status().is_success() {
		return Err("required gate returned an error; no commit published".into());
	}
	let mut bytes = Vec::new();
	while let Some(chunk) = response
		.chunk()
		.await
		.map_err(|_| "required gate body interrupted")?
	{
		if bytes.len() + chunk.len() > 65536 {
			return Err("required gate response exceeds limit".into());
		}
		bytes.extend_from_slice(&chunk);
	}
	let value: Value =
		serde_json::from_slice(&bytes).map_err(|_| "required gate response is not JSON")?;
	let content = value["choices"][0]["message"]["content"]
		.as_str()
		.ok_or("required gate has no verdict")?;
	let verdict: Value =
		serde_json::from_str(content).map_err(|_| "required gate verdict is not JSON")?;
	match verdict["hold"].as_bool() {
		Some(false) => Ok(()),
		Some(true) => Err("required gate held publication".into()),
		None => Err("required gate verdict malformed".into()),
	}
}

fn gate_prompt(diff: &str) -> String {
	format!(
		"Review this diff before commit. Respond ONLY with JSON: {{\"hold\": true|false, \"reason\": \"short reason\", \"subject\": \"one-line commit subject\"}}. Hold only for a real problem (broken build, missing files, obviously wrong).\n\nDIFF:\n{}",
		diff
	)
}

#[cfg(test)]
#[path = "../tests/unit/integrate/tests.rs"]
mod tests;
