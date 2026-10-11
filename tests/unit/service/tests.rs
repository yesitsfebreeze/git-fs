use super::*;
use crate::files;
use futures::FutureExt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tempfile::TempDir;

static NEXT_CALL: AtomicU64 = AtomicU64::new(0);

fn context(cwd: &Path) -> Context {
	Context {
		session: "s1".into(),
		run: "r1".into(),
		call: format!("c{}", NEXT_CALL.fetch_add(1, Ordering::Relaxed)),
		cwd: cwd.to_string_lossy().into_owned(),
	}
}

fn service(config: Config, fail_touch: bool) -> (Arc<Service>, Arc<Mutex<Vec<String>>>, TempDir) {
	let record = Arc::new(Mutex::new(Vec::new()));
	let record_for_touch = record.clone();
	let touch: Touch = Arc::new(move |_ctx, path| {
		let record = record_for_touch.clone();
		async move {
			if fail_touch {
				Err("touch failed".into())
			} else {
				record.lock().unwrap().push(path.target.path);
				Ok(())
			}
		}
		.boxed()
	});
	let service = Arc::new(Service::new(config, touch));
	(service, record, TempDir::new().unwrap())
}

fn call(_tool: &str, ctx: &Context, input: Value) -> Value {
	json!({ "op": "call", "context": ctx, "input": input })
}

#[tokio::test]
async fn whole_read_of_long_file_is_refused_and_establishes_nothing() {
	let (service, _, dir) = service(
		Config {
			max_whole_read_lines: 3,
			..Config::default()
		},
		false,
	);
	let cwd = dir.path();
	std::fs::write(cwd.join("a.txt"), "1\n2\n3\n4\n").unwrap();
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("tool.digest"));
	// refused read recorded nothing: a write still fails
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	// a range passes, as does an exact limit
	for input in [
		json!({ "path": "a.txt", "offset": 2 }),
		json!({ "path": "a.txt", "limit": 2 }),
	] {
		let result = service
			.dispatch("read", call("read", &context(cwd), input))
			.await;
		assert_eq!(result["error"], false, "{result}");
	}
}

#[tokio::test]
async fn unread_overwrite_fails_without_changing_bytes() {
	let (service, record, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "original").unwrap();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "changed" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
	assert!(record.lock().unwrap().is_empty());
}

#[tokio::test]
async fn unseen_edit_fails_without_changing_bytes() {
	let (service, record, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "hello world").unwrap();
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "hello", "new": "goodbye" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
	assert!(record.lock().unwrap().is_empty());
}

#[tokio::test]
async fn stale_version_fails_until_reread_including_same_size_with_restored_mtime() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "abc").unwrap();
	// read establishes the version
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	// external same-size change with restored mtime
	let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
	std::fs::write(&path, "abd").unwrap();
	let file = std::fs::File::options().write(true).open(&path).unwrap();
	file.set_modified(mtime).unwrap();
	// write without reread fails, bytes unchanged
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "abc" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "abd");
	// reread then write succeeds
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "abc" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "abc");
}

#[tokio::test]
async fn concurrent_creators_preserve_the_winner() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	// first unseen write creates
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "first" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	// a second creator in a fresh run is unseen; the existing file must not be clobbered
	let other = Context {
		run: "r2".into(),
		..context(cwd)
	};
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&other,
				json!({ "path": "a.txt", "text": "second" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
}

#[tokio::test]
async fn partial_read_establishes_freshness_only() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
	// partial read with limit 1
	let result = service
		.dispatch(
			"read",
			call(
				"read",
				&context(cwd),
				json!({ "path": "a.txt", "limit": 1 }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	// write succeeds because the version matches
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "new" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
}

#[tokio::test]
async fn pre_cancelled_call_never_mutates() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	let ctx = context(cwd);
	// cancel before registration
	let result = service
		.dispatch("cancel", json!({ "op": "cancel", "context": ctx }))
		.await;
	assert_eq!(result["error"], false);
	// the same unique invocation must not execute
	let result = service
		.dispatch(
			"write",
			call("write", &ctx, json!({ "path": "a.txt", "text": "x" })),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("cancelled"));
	assert!(!path.exists());
}

#[tokio::test]
async fn cancel_during_spawn_prevents_mutation() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	let ctx = context(cwd);
	// register the invocation, then cancel before the worker runs
	let result = service
		.dispatch("cancel", json!({ "op": "cancel", "context": ctx.clone() }))
		.await;
	assert_eq!(result["error"], false);
	let cancel = Arc::new(AtomicBool::new(true));
	let input = files::Input::parse("write", json!({ "path": "a.txt", "text": "x" })).unwrap();
	let outcome = files::execute(&service, &ctx, input, &cancel);
	assert!(outcome.is_err());
	assert!(!path.exists());
}

#[tokio::test]
async fn read_offsets_limits_binary_and_missing() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
	// one-based offset
	let result = service
		.dispatch(
			"read",
			call(
				"read",
				&context(cwd),
				json!({ "path": "a.txt", "offset": 2 }),
			),
		)
		.await;
	let content = result["content"].as_str().unwrap();
	assert!(content.contains("2:two"));
	assert!(!content.contains("1:one"));
	// line limit truncation
	let result = service
		.dispatch(
			"read",
			call(
				"read",
				&context(cwd),
				json!({ "path": "a.txt", "limit": 1 }),
			),
		)
		.await;
	let content = result["content"].as_str().unwrap();
	assert!(content.contains("1:one"));
	assert!(content.contains("truncated"));
	// an offset past the end is an error naming the length, not an empty answer
	let result = service
		.dispatch(
			"read",
			call(
				"read",
				&context(cwd),
				json!({ "path": "a.txt", "offset": 9 }),
			),
		)
		.await;
	assert_eq!(result["error"], true, "{result}");
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("offset 9 is past the end: the file has 3 lines"));
	// binary rejection
	std::fs::write(&path, [0u8, 159, 146, 150]).unwrap();
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("binary"));
	// missing file
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "nope.txt" })),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn read_byte_limit_truncates() {
	let config = Config::parse(json!({ "max_read_bytes": 128 })).unwrap();
	let (service, _, dir) = service(config, false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "x".repeat(500)).unwrap();
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("truncated at 128 bytes"));
}

#[tokio::test]
async fn create_overwrite_and_edit_variants() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	// create
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "hello world" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello world");
	// read then unique edit
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "world", "new": "there" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello there");
	// ambiguous edit without mutation
	std::fs::write(&path, "a a a").unwrap();
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "a", "new": "b" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("ambiguous"));
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "a a a");
	// zero-match rejection
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "zzz", "new": "b" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("no match"));
	// empty-old rejection
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "", "new": "b" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	// all=true replacement
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "a", "new": "b", "all": true }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "b b b");
}

#[tokio::test]
async fn path_escapes_and_forged_context_fail_before_mutation() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	// traversal
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "../escape.txt", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(!cwd.parent().unwrap().join("escape.txt").exists());
	// absolute path outside the workspace
	let outside = TempDir::new().unwrap();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": outside.path().join("x.txt"), "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(!outside.path().join("x.txt").exists());
	// symlinked parent escaping the workspace
	#[cfg(unix)]
	{
		let outside_dir = TempDir::new().unwrap();
		std::os::unix::fs::symlink(outside_dir.path(), cwd.join("link")).unwrap();
		let result = service
			.dispatch(
				"write",
				call(
					"write",
					&context(cwd),
					json!({ "path": "link/x.txt", "text": "x" }),
				),
			)
			.await;
		assert_eq!(result["error"], true);
		assert!(!outside_dir.path().join("x.txt").exists());
	}
	// forged relative cwd
	let forged = Context {
		cwd: "relative/path".into(),
		..context(cwd)
	};
	let result = service
		.dispatch(
			"write",
			call("write", &forged, json!({ "path": "a.txt", "text": "x" })),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(!cwd.join("a.txt").exists());
	// memo record write refused with guidance
	std::fs::create_dir_all(cwd.join(".kern/memos")).unwrap();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": ".kern/memos/x.md", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("tool.memo.write"));
	assert!(!cwd.join(".kern/memos/x.md").exists());
}

#[tokio::test]
async fn touch_records_successful_mutations_only() {
	let (service, record, dir) = service(Config::default(), false);
	let cwd = dir.path();
	// failed write records nothing
	std::fs::write(cwd.join("a.txt"), "original").unwrap();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(record.lock().unwrap().is_empty());
	// successful create records the canonical path
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "b.txt", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	let recorded = record.lock().unwrap().clone();
	assert_eq!(recorded.len(), 1);
	assert!(recorded[0].ends_with("b.txt"));
}

#[tokio::test]
async fn touch_failure_reports_partial_success_with_the_file() {
	let (service, _, dir) = service(Config::default(), true);
	let cwd = dir.path();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(cwd),
				json!({ "path": "a.txt", "text": "x" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	let content = result["content"].as_str().unwrap();
	assert!(content.contains("partial_success"));
	assert!(content.contains("a.txt"));
	// the bytes did change; no rollback is claimed
	assert_eq!(std::fs::read_to_string(cwd.join("a.txt")).unwrap(), "x");
}

#[tokio::test]
async fn failed_preparation_preserves_bytes_and_permissions() {
	let (service, _, dir) = service(Config::default(), false);
	let cwd = dir.path();
	let path = cwd.join("a.txt");
	std::fs::write(&path, "a a a").unwrap();
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
	}
	let result = service
		.dispatch(
			"read",
			call("read", &context(cwd), json!({ "path": "a.txt" })),
		)
		.await;
	assert_eq!(result["error"], false);
	let result = service
		.dispatch(
			"edit",
			call(
				"edit",
				&context(cwd),
				json!({ "path": "a.txt", "old": "a", "new": "b" }),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert_eq!(std::fs::read_to_string(&path).unwrap(), "a a a");
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		assert_eq!(
			std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
			0o600
		);
	}
}

#[tokio::test]
async fn describe_names_and_schemas_match_the_three_keys() {
	let (service, _, _dir) = service(Config::default(), false);
	for tool in ["read", "write", "edit", "search", "glob", "grep"] {
		let result = service.dispatch(tool, json!({ "op": "describe" })).await;
		assert_eq!(result["name"], tool);
		assert!(result["description"].is_string());
		let schema = &result["input_schema"];
		assert_eq!(schema["type"], "object");
		assert!(schema["properties"].is_object());
		assert!(schema["required"].is_array());
		assert_eq!(schema["additionalProperties"], false);
	}
}

#[tokio::test]
async fn newer_bytes_at_commit_survive_write_and_edit() {
	for tool in ["write", "edit"] {
		let (mut service, record, dir) = service(Config::default(), false);
		let path = dir.path().join("a.txt");
		std::fs::write(&path, "original").unwrap();
		Arc::get_mut(&mut service).unwrap().before_commit = Some(Arc::new(|path| {
			std::fs::write(path, "external newer").unwrap();
		}));
		let read = service
			.dispatch(
				"read",
				call("read", &context(dir.path()), json!({"path":"a.txt"})),
			)
			.await;
		assert_eq!(read["error"], false);
		let input = if tool == "write" {
			json!({"path":"a.txt","text":"ours"})
		} else {
			json!({"path":"a.txt","old":"original","new":"ours"})
		};
		let result = service
			.dispatch(tool, call(tool, &context(dir.path()), input))
			.await;
		assert_eq!(
			std::fs::read_to_string(&path).unwrap(),
			"external newer",
			"{tool} overwrote the external change: {result}"
		);
		assert_eq!(result["error"], true);
		assert!(
			result["content"].as_str().unwrap().contains("conflict"),
			"{result}"
		);
		assert!(record.lock().unwrap().is_empty());
		assert_eq!(
			std::fs::read_dir(dir.path()).unwrap().count(),
			1,
			"no abandoned temp"
		);
	}
}

#[tokio::test]
async fn disappearance_and_creation_at_publication_are_conflicts() {
	for existed in [false, true] {
		let (mut service, record, dir) = service(Config::default(), false);
		let path = dir.path().join("a.txt");
		if existed {
			std::fs::write(&path, "original").unwrap();
		}
		Arc::get_mut(&mut service).unwrap().before_commit = Some(Arc::new(move |path| {
			if existed {
				std::fs::remove_file(path).unwrap();
			} else {
				std::fs::write(path, "another creator").unwrap();
			}
		}));
		service
			.dispatch(
				"read",
				call("read", &context(dir.path()), json!({"path":"a.txt"})),
			)
			.await;
		let result = service
			.dispatch(
				"write",
				call(
					"write",
					&context(dir.path()),
					json!({"path":"a.txt","text":"ours"}),
				),
			)
			.await;
		assert_eq!(result["error"], true, "{result}");
		assert!(result["content"].as_str().unwrap().contains("conflict"));
		if existed {
			assert!(!path.exists());
		} else {
			assert_eq!(std::fs::read_to_string(path).unwrap(), "another creator");
		}
		assert!(record.lock().unwrap().is_empty());
		assert_eq!(
			std::fs::read_dir(dir.path()).unwrap().count(),
			usize::from(!existed)
		);
	}
}

#[test]
fn cancellation_after_preparation_leaves_no_mutation_or_temp() {
	for tool in ["write", "edit"] {
		let (mut service, _, dir) = service(Config::default(), false);
		let path = dir.path().join("a.txt");
		std::fs::write(&path, "original").unwrap();
		let cancel = Arc::new(AtomicBool::new(false));
		let hook_cancel = cancel.clone();
		Arc::get_mut(&mut service).unwrap().before_commit = Some(Arc::new(move |_| {
			hook_cancel.store(true, Ordering::SeqCst);
		}));
		let ctx = context(dir.path());
		files::execute(
			&service,
			&ctx,
			files::Input::parse("read", json!({"path":"a.txt"})).unwrap(),
			&cancel,
		)
		.unwrap();
		let input = if tool == "write" {
			json!({"path":"a.txt","text":"ours"})
		} else {
			json!({"path":"a.txt","old":"original","new":"ours"})
		};
		let result = files::execute(
			&service,
			&ctx,
			files::Input::parse(tool, input).unwrap(),
			&cancel,
		);
		assert_eq!(result.err().unwrap(), "cancelled");
		assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
		assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
	}
}

#[test]
fn competing_sessions_with_one_read_revision_have_one_winner() {
	let (service, _, dir) = service(Config::default(), false);
	let path = dir.path().join("a.txt");
	std::fs::write(&path, "original").unwrap();
	let start = Arc::new(std::sync::Barrier::new(2));
	let mut writers = Vec::new();
	for index in 0..2 {
		let ctx = Context {
			session: format!("session-{index}"),
			..context(dir.path())
		};
		files::execute(
			&service,
			&ctx,
			files::Input::parse("read", json!({"path":"a.txt"})).unwrap(),
			&AtomicBool::new(false),
		)
		.unwrap();
		let service = service.clone();
		let start = start.clone();
		writers.push(std::thread::spawn(move || {
			start.wait();
			files::execute(
				&service,
				&ctx,
				files::Input::parse(
					"write",
					json!({"path":"a.txt","text":format!("writer-{index}")}),
				)
				.unwrap(),
				&AtomicBool::new(false),
			)
		}));
	}
	let results: Vec<_> = writers.into_iter().map(|w| w.join().unwrap()).collect();
	assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
	assert!(results
		.iter()
		.find_map(|r| r.as_ref().err())
		.unwrap()
		.contains("changed since"));
	assert!(std::fs::read_to_string(path)
		.unwrap()
		.starts_with("writer-"));
	assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn commit_rechecks_symlinks_and_preserves_executable_modes() {
	use std::os::unix::fs::{symlink, PermissionsExt};
	for escape in [true, false] {
		for tool in ["write", "edit"] {
			let (mut service, record, dir) = service(Config::default(), false);
			let outside = TempDir::new().unwrap();
			let foreign = outside.path().join("protected");
			std::fs::write(&foreign, "original").unwrap();
			let path = dir.path().join("a.txt");
			std::fs::write(&path, "original").unwrap();
			std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o751)).unwrap();
			if escape {
				let foreign = foreign.clone();
				Arc::get_mut(&mut service).unwrap().before_commit = Some(Arc::new(move |path| {
					std::fs::remove_file(path).unwrap();
					symlink(&foreign, path).unwrap();
				}));
			}
			service
				.dispatch(
					"read",
					call("read", &context(dir.path()), json!({"path":"a.txt"})),
				)
				.await;
			let input = if tool == "write" {
				json!({"path":"a.txt","text":"ours"})
			} else {
				json!({"path":"a.txt","old":"original","new":"ours"})
			};
			let result = service
				.dispatch(tool, call(tool, &context(dir.path()), input))
				.await;
			assert_eq!(result["error"], escape, "{result}");
			assert_eq!(std::fs::read_to_string(&foreign).unwrap(), "original");
			if escape {
				assert!(path.is_symlink());
				assert!(record.lock().unwrap().is_empty());
			} else {
				assert_eq!(std::fs::read_to_string(&path).unwrap(), "ours");
				assert_eq!(
					std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
					0o751
				);
			}
			assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
		}
	}
}

#[tokio::test]
async fn read_record_failure_after_publication_is_explicit_partial_success() {
	let (mut service, record, dir) = service(
		Config {
			max_reads: 1,
			..Config::default()
		},
		false,
	);
	let ctx = context(dir.path());
	let holder = Arc::new(Mutex::new(std::sync::Weak::<Service>::new()));
	let hook_holder = holder.clone();
	let hook_context = ctx.clone();
	Arc::get_mut(&mut service).unwrap().before_commit = Some(Arc::new(move |path| {
		hook_holder
			.lock()
			.unwrap()
			.upgrade()
			.unwrap()
			.mark_read(&hook_context, &path.with_file_name("other"), Read::Absent)
			.unwrap();
	}));
	*holder.lock().unwrap() = Arc::downgrade(&service);
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&ctx,
				json!({"path":"a.txt","text":"complete new bytes"}),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	let content = result["content"].as_str().unwrap();
	assert!(
		content.contains("partial_success")
			&& content.contains("a.txt")
			&& content.contains("Do not automatically retry"),
		"{result}"
	);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
		"complete new bytes"
	);
	assert_eq!(record.lock().unwrap().len(), 1);
	assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

fn evidence_service() -> (Arc<Service>, Arc<Mutex<Vec<Evidence>>>, TempDir) {
	let records = Arc::new(Mutex::new(Vec::new()));
	let copy = records.clone();
	let callback: Touch = Arc::new(move |_, evidence| {
		let copy = copy.clone();
		async move {
			copy.lock().unwrap().push(evidence);
			Ok(())
		}
		.boxed()
	});
	(
		Arc::new(Service::new(Config::default(), callback)),
		records,
		TempDir::new().unwrap(),
	)
}
#[tokio::test]
async fn direct_evidence_uses_guarded_bytes_and_unique_publication_identity() {
	use crate::change_record::{Operation, Provider, Storage, Version};
	use sha2::{Digest, Sha256};
	let (service, records, dir) = evidence_service();
	let mut ctx = context(dir.path());
	for (index, body) in ["alpha", "alpha"].into_iter().enumerate() {
		ctx.call = format!("publication-{index}");
		assert_eq!(
			service
				.dispatch(
					"write",
					call("write", &ctx, json!({"path":"a","text":body}))
				)
				.await["error"],
			false
		);
	}
	ctx.call = "publication-edit".into();
	assert_eq!(
		service
			.dispatch(
				"edit",
				call("edit", &ctx, json!({"path":"a","old":"alpha","new":"beta"}))
			)
			.await["error"],
		false
	);
	let rows = records.lock().unwrap().clone();
	assert_eq!(rows.len(), 3);
	let alpha = Version::Sha256 {
		value: format!("{:x}", Sha256::digest(b"alpha")),
	};
	assert_eq!(rows[0].before, Version::Absent {});
	assert_eq!(rows[0].after, alpha);
	assert_eq!(rows[1].before, alpha);
	assert_eq!(rows[1].after, alpha);
	assert_ne!(rows[0].publication_id, rows[1].publication_id);
	assert_eq!(rows[2].before, alpha);
	assert_eq!(
		rows[2].after,
		Version::Sha256 {
			value: format!("{:x}", Sha256::digest(b"beta"))
		}
	);
	assert_eq!(rows[2].operation, Operation::Edit);
	for row in &rows {
		row.validate(&ctx.session).unwrap();
		assert_eq!(row.provider, Provider::Fs);
		assert_eq!(row.target.storage, Storage::Direct);
		assert_eq!(row.actor.run, Some(ctx.run.clone()));
		assert!(row
			.actor
			.call
			.as_deref()
			.unwrap()
			.starts_with("publication-"));
		assert_eq!(
			row.target.workspace,
			dir.path().canonicalize().unwrap().to_str().unwrap()
		);
		assert_eq!(
			row.target.path,
			dir.path()
				.join("a")
				.canonicalize()
				.unwrap()
				.to_str()
				.unwrap()
		);
		assert!(row.target.overlay_ref.is_none());
	}
	ctx.call = "stale-write".into();
	// External modification must not generate an evidence record, nor may a stale write.
	std::fs::write(dir.path().join("a"), "external").unwrap();
	assert_eq!(
		service
			.dispatch(
				"write",
				call("write", &ctx, json!({"path":"a","text":"no"}))
			)
			.await["error"],
		true
	);
	assert_eq!(records.lock().unwrap().len(), 3);
}
#[tokio::test]
async fn failed_cleanup_after_known_publication_still_records_once() {
	let (mut service, records, dir) = evidence_service();
	Arc::get_mut(&mut service).unwrap().after_publish = Some(Arc::new(|path| {
		for entry in std::fs::read_dir(path.parent().unwrap()).unwrap() {
			let path = entry.unwrap().path();
			if path.extension().is_some_and(|ext| ext == "tmp") {
				std::fs::remove_file(path).unwrap();
			}
		}
	}));
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(dir.path()),
				json!({"path":"a","text":"published"}),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("temp cleanup failed"));
	assert_eq!(
		std::fs::read_to_string(dir.path().join("a")).unwrap(),
		"published"
	);
	assert_eq!(records.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn recording_timeout_is_bounded_unknown_and_never_replays_file_publication() {
	let attempts = Arc::new(AtomicU64::new(0));
	let copy = attempts.clone();
	let callback: Touch = Arc::new(move |_, _| {
		copy.fetch_add(1, Ordering::Relaxed);
		futures::future::pending().boxed()
	});
	let service = Arc::new(Service::new(Config::default(), callback));
	let dir = TempDir::new().unwrap();
	let start = std::time::Instant::now();
	let result = service
		.dispatch(
			"write",
			call(
				"write",
				&context(dir.path()),
				json!({"path":"a","text":"published"}),
			),
		)
		.await;
	assert!(start.elapsed() < std::time::Duration::from_secs(4));
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("recording outcome unknown"));
	assert_eq!(attempts.load(Ordering::Relaxed), 1);
	assert_eq!(
		std::fs::read_to_string(dir.path().join("a")).unwrap(),
		"published"
	);
}
