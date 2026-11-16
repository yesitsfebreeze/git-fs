//! Real stdio MCP processes sharing Git refs, with an explicit replayable RPC schedule.
//! The harness deliberately never uses sleeps to choose a winner.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Barrier};
use std::time::{Duration, Instant};

const RPC_TIMEOUT: Duration = Duration::from_secs(20);

struct Peer {
	child: Child,
	input: ChildStdin,
	replies: mpsc::Receiver<String>,
	next_id: u64,
	name: String,
	latencies: Vec<(String, u128)>,
	refused: usize,
}

impl Peer {
	fn start(root: &Path, name: &str) -> Self {
		Self::start_env(root, name, &[])
	}

	fn start_env(root: &Path, name: &str, env: &[(&str, &str)]) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_git-fs"))
			.envs(env.iter().copied())
			.arg("mcp")
			.current_dir(root)
			.env("GIT_FS_DISK", root)
			.env("GIT_FS_CONFIG", "{}")
			.env("GIT_FS_COORDINATED", "1")
			.env("GIT_FS_SESSION", name)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::inherit())
			.spawn()
			.unwrap();
		let input = child.stdin.take().unwrap();
		let output = child.stdout.take().unwrap();
		let (tx, replies) = mpsc::channel();
		std::thread::spawn(move || {
			for line in BufReader::new(output).lines() {
				if tx.send(line.unwrap()).is_err() {
					break;
				}
			}
		});
		let mut peer = Self {
			child,
			input,
			replies,
			next_id: 0,
			name: name.into(),
			latencies: Vec::new(),
			refused: 0,
		};
		let init = peer.request(
			"initialize",
			json!({
				"protocolVersion": "2025-06-18",
				"capabilities": {},
				"clientInfo": {"name": name, "version": "1"}
			}),
		);
		assert!(init.get("error").is_none(), "{name}: {init}");
		peer
	}

	fn request(&mut self, method: &str, params: Value) -> Value {
		let operation = params["name"].as_str().unwrap_or(method).to_owned();
		let started = Instant::now();
		self.next_id += 1;
		let message = json!({
			"jsonrpc": "2.0", "id": self.next_id,
			"method": method, "params": params
		});
		writeln!(self.input, "{message}").unwrap();
		self.input.flush().unwrap();
		let line = self
			.replies
			.recv_timeout(RPC_TIMEOUT)
			.unwrap_or_else(|error| {
				panic!(
				"{} pid {} timed out/disconnected at {method} id {}: {error}; request {message}",
				self.name,
				self.child.id(),
				self.next_id
			)
			});
		let response: Value = serde_json::from_str(&line).unwrap();
		assert_eq!(response["id"], self.next_id, "{}: {response}", self.name);
		self.latencies
			.push((operation, started.elapsed().as_micros()));
		if response["result"]["isError"] == true {
			self.refused += 1;
		}
		response
	}

	fn call(&mut self, tool: &str, input: Value) -> Value {
		self.request("tools/call", json!({"name": tool, "arguments": input}))["result"].clone()
	}

	fn ok(&mut self, tool: &str, input: Value) -> Value {
		let result = self.call(tool, input);
		assert_eq!(result["isError"], false, "{} {tool}: {result}", self.name);
		result
	}

	fn assert_identity(&mut self) {
		let result = content(&self.ok("gitfs_ls", json!({})));
		assert_eq!(result["runtime"]["session"]["id"], self.name);
	}
}

impl Drop for Peer {
	fn drop(&mut self) {
		let _ = self.child.kill();
		let _ = self.child.wait();
		let mut micros: Vec<_> = self.latencies.iter().map(|(_, us)| *us).collect();
		if !micros.is_empty() {
			micros.sort_unstable();
			let n = micros.len();
			eprintln!(
				"gitfs-gfs06-baseline os={} peer={} samples={} p50_us={} max_us={} refused={} operations={:?}",
				std::env::consts::OS,
				self.name,
				n,
				micros[n / 2],
				micros[n - 1],
				self.refused,
				self.latencies
			);
		}
	}
}

fn content(result: &Value) -> Value {
	serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn git(root: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.args(args)
		.current_dir(root)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().into()
}

fn repo() -> (tempfile::TempDir, PathBuf) {
	let dir = tempfile::tempdir().unwrap();
	let root = dir.path().canonicalize().unwrap();
	git(&root, &["init", "-q"]);
	git(&root, &["config", "user.name", "Concurrent Test"]);
	git(&root, &["config", "user.email", "test@example.com"]);
	git(&root, &["config", "commit.gpgsign", "false"]);
	for name in ["alpha.txt", "beta.txt", "obsolete.txt"] {
		std::fs::write(root.join(name), "before\n").unwrap();
	}
	git(&root, &["add", "."]);
	git(&root, &["commit", "-qm", "base"]);
	(dir, root)
}

fn preview(peer: &mut Peer, message: &str) -> Value {
	content(&peer.ok("integrate", json!({"phase":"preview","message":message})))
}

fn commit(peer: &mut Peer, message: &str) {
	let candidate = preview(peer, message);
	peer.ok(
		"integrate",
		json!({"phase":"commit", "message":message, "expected_revision":candidate["revision"]}),
	);
}

#[test]
fn two_coordinated_processes_publish_disjoint_changes_and_preserve_index() {
	let (_dir, root) = repo();
	std::fs::write(root.join("sentinel.txt"), "staged\n").unwrap();
	git(&root, &["add", "sentinel.txt"]);
	std::fs::write(root.join("sentinel.txt"), "unstaged\n").unwrap();
	let sentinel_stage = git(&root, &["ls-files", "--stage", "--", "sentinel.txt"]);
	let base = git(&root, &["rev-parse", "HEAD"]);
	let original_index = std::fs::read(root.join(".git/index")).unwrap();
	let mut a = Peer::start(&root, "agent-a");
	let mut b = Peer::start(&root, "agent-b");
	a.assert_identity();
	b.assert_identity();
	a.ok("read", json!({"path":"alpha.txt"}));
	b.ok("read", json!({"path":"beta.txt"}));

	// Schedule: both clients observe the base, then an explicit three-party
	// barrier releases both independent MCP processes to publish their refs.
	let gate = Arc::new(Barrier::new(3));
	std::thread::scope(|scope| {
		let gate_a = Arc::clone(&gate);
		let gate_b = Arc::clone(&gate);
		let a_ref = &mut a;
		let ta = scope.spawn(move || {
			gate_a.wait();
			a_ref.assert_identity();
			a_ref.ok(
				"edit",
				json!({"path":"alpha.txt","old":"before","new":"from-a"}),
			);
		});
		let b_ref = &mut b;
		let tb = scope.spawn(move || {
			gate_b.wait();
			b_ref.assert_identity();
			b_ref.ok(
				"edit",
				json!({"path":"beta.txt","old":"before","new":"from-b"}),
			);
		});
		gate.wait();
		ta.join().unwrap();
		tb.join().unwrap();
	});
	assert_eq!(
		git(&root, &["show", "refs/gitfs/agent-a:alpha.txt"]),
		"from-a"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs/agent-b:beta.txt"]),
		"from-b"
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	assert_eq!(
		std::fs::read(root.join(".git/index")).unwrap(),
		original_index
	);
	assert_eq!(
		std::fs::read_to_string(root.join("alpha.txt")).unwrap(),
		"before\n"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("beta.txt")).unwrap(),
		"before\n"
	);

	// Deliberate order is replayable, and both changes must survive integration.
	a.assert_identity();
	commit(&mut a, "publish alpha");
	b.assert_identity();
	commit(&mut b, "publish beta");
	assert_eq!(git(&root, &["show", "HEAD:alpha.txt"]), "from-a");
	assert_eq!(git(&root, &["show", "HEAD:beta.txt"]), "from-b");
	assert_eq!(
		git(&root, &["ls-files", "--stage", "--", "sentinel.txt"]),
		sentinel_stage
	);
	b.assert_identity();
	b.ok("gitfs_materialize", json!({"paths":["beta.txt"]}));
	assert_eq!(
		std::fs::read_to_string(root.join("beta.txt")).unwrap(),
		"from-b\n"
	);
	assert_eq!(
		git(&root, &["ls-files", "--stage", "--", "sentinel.txt"]),
		sentinel_stage
	);
	assert_eq!(
		std::fs::read_to_string(root.join("sentinel.txt")).unwrap(),
		"unstaged\n"
	);
}

#[test]
fn same_line_stale_preimage_is_refused_before_publication() {
	let (_dir, root) = repo();
	let mut a = Peer::start(&root, "conflict-a");
	let mut b = Peer::start(&root, "conflict-b");
	a.assert_identity();
	b.assert_identity();
	a.ok("read", json!({"path":"alpha.txt"}));
	b.ok("read", json!({"path":"alpha.txt"}));
	let base = git(&root, &["rev-parse", "HEAD"]);

	// Replayable causal order: both read base, A publishes, then B attempts
	// a stale write to the exact same lines. B must not touch ANY ref or disk.
	a.assert_identity();
	a.ok(
		"edit",
		json!({"path":"alpha.txt","old":"before","new":"winner"}),
	);
	let refs_before = git(&root, &["show-ref"]);
	let index_before = std::fs::read(root.join(".git/index")).unwrap();
	b.assert_identity();
	let refused = b.call(
		"edit",
		json!({"path":"alpha.txt","old":"before","new":"loser"}),
	);
	assert_eq!(refused["isError"], true, "{refused}");
	assert!(refused["content"][0]["text"]
		.as_str()
		.unwrap()
		.contains("reread"));
	assert_eq!(git(&root, &["show-ref"]), refs_before);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	assert_eq!(
		std::fs::read(root.join(".git/index")).unwrap(),
		index_before
	);
	assert_eq!(
		std::fs::read_to_string(root.join("alpha.txt")).unwrap(),
		"before\n"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs/conflict-a:alpha.txt"]),
		"winner"
	);
	commit(&mut a, "win alpha");
	assert_eq!(git(&root, &["show", "HEAD:alpha.txt"]), "winner");
}

#[test]
fn restarted_peer_reuses_published_ref_and_keeps_integration_idempotent() {
	let (_dir, root) = repo();
	let base = git(&root, &["rev-parse", "HEAD"]);
	let original_index = std::fs::read(root.join(".git/index")).unwrap();
	let mut first = Peer::start(&root, "resumable");
	first.assert_identity();
	first.ok("read", json!({"path":"alpha.txt"}));
	first.ok(
		"edit",
		json!({"path":"alpha.txt","old":"before","new":"persistent"}),
	);
	let branch = git(&root, &["rev-parse", "refs/gitfs/resumable"]);
	assert_eq!(
		git(&root, &["show", "refs/gitfs/resumable:alpha.txt"]),
		"persistent"
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	assert_eq!(
		std::fs::read(root.join(".git/index")).unwrap(),
		original_index
	);
	drop(first); // An OS process dies after publishing its session ref.

	let mut resumed = Peer::start(&root, "resumable");
	resumed.assert_identity();
	assert_eq!(git(&root, &["rev-parse", "refs/gitfs/resumable"]), branch);
	let candidate = preview(&mut resumed, "resume exact candidate");
	let input = json!({
		"phase":"commit", "expected_revision":candidate["revision"],
		"message":"resume exact candidate"
	});
	resumed.ok("integrate", input.clone());
	let integrated = git(&root, &["rev-parse", "HEAD"]);
	assert_ne!(integrated, base);
	assert_eq!(git(&root, &["show", "HEAD:alpha.txt"]), "persistent");
	let replay = content(&resumed.ok("integrate", input));
	assert_eq!(replay["committed"], integrated, "{replay}");
	assert_eq!(replay["outcome"]["publication"], "committed", "{replay}");
	let reconciliation = content(&resumed.ok(
		"integrate",
		json!({"phase":"reconcile","expected_revision":candidate["revision"]}),
	));
	assert_eq!(reconciliation["committed"], integrated, "{reconciliation}");
	assert_eq!(
		git(&root, &["rev-parse", "HEAD"]),
		integrated,
		"replay made a second commit"
	);

	// Explicit manual disk capture may change only the selected branch paths.
	std::fs::write(root.join("manual.txt"), "captured\n").unwrap();
	resumed.assert_identity();
	resumed.ok("gitfs_capture", json!({"paths":["manual.txt"]}));
	commit(&mut resumed, "capture manual disk state");
	assert_eq!(git(&root, &["show", "HEAD:manual.txt"]), "captured");
	resumed.assert_identity();
	resumed.ok("read", json!({"path":"obsolete.txt"}));
	resumed.ok("delete", json!({"path":"obsolete.txt"}));
	commit(&mut resumed, "delete obsolete");
	assert_eq!(
		git(
			&root,
			&["ls-tree", "--name-only", "HEAD", "--", "obsolete.txt"]
		),
		""
	);
}

#[test]
fn stale_candidate_and_path_denial_never_advance_head() {
	let (_dir, root) = repo();
	let mut peer = Peer::start(&root, "candidate");
	let base = git(&root, &["rev-parse", "HEAD"]);
	let index = std::fs::read(root.join(".git/index")).unwrap();

	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	peer.assert_identity();
	peer.ok("read", json!({"path":"alpha.txt"}));
	peer.ok(
		"edit",
		json!({"path":"alpha.txt","old":"before","new":"first"}),
	);
	let stale = preview(&mut peer, "stale candidate");
	peer.ok(
		"edit",
		json!({"path":"alpha.txt","old":"first","new":"second"}),
	);
	let latest_ref = git(&root, &["rev-parse", "refs/gitfs/candidate"]);
	let rejected = peer.call(
		"integrate",
		json!({"phase":"commit","expected_revision":stale["revision"],"message":"stale candidate"}),
	);
	assert_eq!(rejected["isError"], true, "{rejected}");
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	assert_eq!(
		git(&root, &["rev-parse", "refs/gitfs/candidate"]),
		latest_ref
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs/candidate:alpha.txt"]),
		"second"
	);
	assert_eq!(std::fs::read(root.join(".git/index")).unwrap(), index);
	assert_eq!(
		std::fs::read_to_string(root.join("alpha.txt")).unwrap(),
		"before\n"
	);

	let refs = git(&root, &["show-ref"]);
	let forbidden = peer.call("read", json!({"path":"../outside.txt"}));
	assert_eq!(forbidden["isError"], true, "{forbidden}");
	assert_eq!(git(&root, &["show-ref"]), refs);
}

#[test]
fn two_peer_candidate_snapshots_are_exact_isolated_and_capability_scoped() {
	let (_dir, root) = repo();
	let mut a = Peer::start(&root, "snapshot-a");
	let mut b = Peer::start(&root, "snapshot-b");
	a.assert_identity();
	b.assert_identity();
	a.ok("read", json!({"path":"alpha.txt"}));
	b.ok("read", json!({"path":"beta.txt"}));
	a.ok(
		"edit",
		json!({"path":"alpha.txt","old":"before","new":"candidate-a"}),
	);
	b.ok(
		"edit",
		json!({"path":"beta.txt","old":"before","new":"candidate-b"}),
	);
	let a_review = preview(&mut a, "candidate a");
	let b_review = preview(&mut b, "candidate b");
	let a_source = &a_review["tree"]["source"];
	let b_source = &b_review["tree"]["source"];
	assert!(
		a_source.is_object() && a_source["tree"].is_string(),
		"{a_review}"
	);
	assert!(
		b_source.is_object() && b_source["tree"].is_string(),
		"{b_review}"
	);
	let a_snapshot = content(&a.ok(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "expected_tree":a_source["tree"],
			"expected_review":a_source
		}),
	));
	let b_snapshot = content(&b.ok(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "expected_tree":b_source["tree"],
			"expected_review":b_source
		}),
	));
	assert_eq!(a_snapshot["tree"], a_source["tree"]);
	assert_eq!(b_snapshot["tree"], b_source["tree"]);
	assert_ne!(a_snapshot["capability"], b_snapshot["capability"]);
	assert_ne!(a_snapshot["path"], b_snapshot["path"]);
	for snapshot in [&a_snapshot, &b_snapshot] {
		let checkout = PathBuf::from(snapshot["path"].as_str().expect("candidate checkout path"));
		let manifest = PathBuf::from(
			snapshot["manifest"]
				.as_str()
				.expect("candidate manifest path"),
		);
		assert!(
			manifest.is_file(),
			"missing candidate manifest: {}",
			manifest.display()
		);
		assert_eq!(
			manifest.parent().unwrap().canonicalize().unwrap(),
			checkout.parent().unwrap().canonicalize().unwrap()
		);
		let record: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
		assert_eq!(record["schema"], "gitfs-candidate-snapshot/v1");
		assert_eq!(record["id"], snapshot["id"]);
		assert_eq!(record["path"], snapshot["path"]);
		assert_eq!(record["commit"], snapshot["revision"]);
		assert_eq!(record["review"]["tree"], snapshot["tree"]);
		assert_eq!(
			record["capability_hash"].as_str().map(str::len),
			Some(64),
			"candidate capability digest"
		);
	}
	let a_path = PathBuf::from(a_snapshot["path"].as_str().unwrap());
	let b_path = PathBuf::from(b_snapshot["path"].as_str().unwrap());
	assert_eq!(
		git(&a_path, &["rev-parse", "HEAD^{tree}"]),
		a_snapshot["tree"]
	);
	assert_eq!(
		git(&b_path, &["rev-parse", "HEAD^{tree}"]),
		b_snapshot["tree"]
	);
	assert_eq!(git(&a_path, &["show", "HEAD:alpha.txt"]), "candidate-a");
	assert_eq!(git(&b_path, &["show", "HEAD:beta.txt"]), "candidate-b");

	a.assert_identity();
	a.ok(
		"edit",
		json!({"path":"alpha.txt","old":"candidate-a","new":"next"}),
	);
	let stale = a.call(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "expected_tree":a_source["tree"]
		}),
	);
	assert_eq!(stale["isError"], true, "{stale}");
	assert!(
		stale["content"][0]["text"]
			.as_str()
			.is_some_and(|s| s.contains("candidate tree changed since preview")),
		"{stale}"
	);
	assert_eq!(git(&a_path, &["show", "HEAD:alpha.txt"]), "candidate-a");
	let wrong_owner = b.call(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "id":a_snapshot["id"], "capability":a_snapshot["capability"]
		}),
	);
	assert_eq!(wrong_owner["isError"], true, "{wrong_owner}");
	assert!(
		wrong_owner["content"][0]["text"]
			.as_str()
			.is_some_and(|s| s.contains("different session")),
		"{wrong_owner}"
	);
	a.ok(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "id":a_snapshot["id"], "capability":a_snapshot["capability"]
		}),
	);
	b.ok(
		"gitfs_snapshot",
		json!({
			"kind":"candidate", "id":b_snapshot["id"], "capability":b_snapshot["capability"]
		}),
	);
	assert!(!a_path.exists());
	assert!(!b_path.exists());
}

/// Lines `<pid> <point> <pass|wait>` the server's test-only checkpoints wrote.
fn trace(dir: &Path) -> Vec<String> {
	std::fs::read_to_string(dir.join("trace"))
		.unwrap_or_default()
		.lines()
		.map(str::to_owned)
		.collect()
}

/// Event wait, not a timing choice: returns once the server reports `line`.
fn await_trace(dir: &Path, line: &str) {
	let deadline = Instant::now() + RPC_TIMEOUT;
	while !trace(dir).iter().any(|l| l == line) {
		assert!(
			Instant::now() < deadline,
			"never reached {line}: {:?}",
			trace(dir)
		);
		std::thread::yield_now();
	}
}

fn release(dir: &Path, point: &str, pid: u32) {
	std::fs::write(dir.join(format!("{point}.{pid}.go")), "").unwrap();
}

fn barrier_peers(root: &Path, barrier: &Path, pause: &str) -> (Peer, Peer) {
	let dir = barrier.to_str().unwrap();
	let a = Peer::start_env(
		root,
		"barrier-a",
		&[
			("GITFS_TEST_BARRIER_DIR", dir),
			("GITFS_TEST_BARRIER_PAUSE", pause),
		],
	);
	let b = Peer::start_env(root, "barrier-b", &[("GITFS_TEST_BARRIER_DIR", dir)]);
	(a, b)
}

#[test]
fn barrier_replays_peer_publication_between_preimage_check_and_live_recheck() {
	let (_dir, root) = repo();
	let barrier = tempfile::tempdir().unwrap();
	let (mut a, mut b) = barrier_peers(&root, barrier.path(), "preimage");
	let (pa, pb) = (a.child.id(), b.child.id());
	a.ok("read", json!({"path":"alpha.txt"}));
	b.ok("read", json!({"path":"alpha.txt"}));
	let base = git(&root, &["rev-parse", "HEAD"]);

	// Schedule: A passes its handler preimage check and stops before the
	// locked live re-check; B publishes the same line; A resumes.
	let refused = std::thread::scope(|scope| {
		let a_edit = scope.spawn(|| {
			a.call(
				"edit",
				json!({"path":"alpha.txt","old":"before","new":"loser"}),
			)
		});
		await_trace(barrier.path(), &format!("{pa} preimage wait"));
		b.ok(
			"edit",
			json!({"path":"alpha.txt","old":"before","new":"winner"}),
		);
		release(barrier.path(), "preimage", pa);
		a_edit.join().unwrap()
	});
	assert_eq!(refused["isError"], true, "{refused}");
	assert!(
		refused["content"][0]["text"]
			.as_str()
			.unwrap()
			.contains("live conflict"),
		"{refused}"
	);
	let order: Vec<_> = trace(barrier.path())
		.into_iter()
		.filter(|l| l.contains(" preimage ") || l.contains(" publish "))
		.collect();
	assert_eq!(
		order,
		[
			format!("{pa} preimage wait"),
			format!("{pb} preimage pass"),
			format!("{pb} publish pass"),
		],
		"A must never reach publication"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:alpha.txt"]),
		"winner"
	);
	assert_eq!(git(&root, &["rev-parse", "HEAD"]), base);
	assert_eq!(
		std::fs::read_to_string(root.join("alpha.txt")).unwrap(),
		"before\n"
	);
}

#[test]
fn barrier_holds_peer_behind_locked_publication_then_both_disjoint_edits_land() {
	let (_dir, root) = repo();
	let barrier = tempfile::tempdir().unwrap();
	let (mut a, mut b) = barrier_peers(&root, barrier.path(), "publish");
	let (pa, pb) = (a.child.id(), b.child.id());
	a.ok("read", json!({"path":"alpha.txt"}));
	b.ok("read", json!({"path":"beta.txt"}));

	// Schedule: A holds the repository lock at its ref publication; B's
	// disjoint edit reaches the lock and must wait; A publishes, then B.
	std::thread::scope(|scope| {
		let a_edit =
			scope.spawn(|| a.ok("edit", json!({"path":"alpha.txt","old":"before","new":"a"})));
		await_trace(barrier.path(), &format!("{pa} publish wait"));
		let b_edit =
			scope.spawn(|| b.ok("edit", json!({"path":"beta.txt","old":"before","new":"b"})));
		await_trace(barrier.path(), &format!("{pb} preimage pass"));
		assert!(!trace(barrier.path()).contains(&format!("{pb} publish pass")));
		release(barrier.path(), "publish", pa);
		a_edit.join().unwrap();
		b_edit.join().unwrap();
	});
	let publishes: Vec<_> = trace(barrier.path())
		.into_iter()
		.filter(|l| l.contains(" publish "))
		.collect();
	assert_eq!(
		publishes,
		[format!("{pa} publish wait"), format!("{pb} publish pass")]
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:alpha.txt"]),
		"a"
	);
	assert_eq!(
		git(&root, &["show", "refs/gitfs-live/current:beta.txt"]),
		"b"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("beta.txt")).unwrap(),
		"before\n"
	);
}
