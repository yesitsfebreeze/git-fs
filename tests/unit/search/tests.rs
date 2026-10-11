//! Composition, backend isolation, cancellation, and honesty tests for the
//! composable search tools. Uses a fake `rg` executable for backend cases so
//! real ripgrep behavior is not assumed; real `rg` is exercised in the
//! temporary-tree tests only where it is installed.

use super::*;
use crate::service::{self, Config, Service};
use futures::FutureExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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

fn service(config: Config) -> (Arc<Service>, TempDir) {
	let touch: service::Touch = Arc::new(|_ctx, _path| async { Ok(()) }.boxed());
	let service = Arc::new(Service::new(config, touch));
	(service, TempDir::new().unwrap())
}

fn call(ctx: &Context, input: Value) -> Value {
	json!({ "op": "call", "context": ctx, "input": input })
}

/// Write a fake `rg` shell script into the temp dir and return its path.
/// The script prints `$@` (its arguments) and exits with the given code, so
/// tests can assert argv shapes and exit handling without real ripgrep.
fn fake_rg(dir: &Path, body: &str) -> PathBuf {
	let script = dir.join("rg");
	std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
	use std::os::unix::fs::PermissionsExt;
	let mut perms = std::fs::metadata(&script).unwrap().permissions();
	perms.set_mode(0o755);
	std::fs::set_permissions(&script, perms).unwrap();
	script
}

fn cfg_with_rg(dir: &Path) -> Config {
	Config {
		rg_program: dir.join("rg").to_string_lossy().into_owned(),
		// The default 10s deadline, not a tight one: it covers process spawn,
		// and a loaded machine needs far more than the fake script's own cost.
		// The deadline itself is exercised by the test that sets its own.
		..Config::default()
	}
}

#[tokio::test]
async fn empty_stages_is_an_empty_set_never_a_sweep() {
	let (service, dir) = service(Config::default());
	let cwd = dir.path();
	std::fs::write(cwd.join("a.txt"), "hello").unwrap();
	let result = service
		.dispatch("search", call(&context(cwd), json!({"stages": []})))
		.await;
	assert_eq!(result["error"], false);
	let content: Value = result["content"].clone();
	assert_eq!(content["kind"], "paths");
	assert_eq!(content["items"].as_array().unwrap().len(), 0);
	assert_eq!(content["complete"], true);
}

#[tokio::test]
async fn incompatible_stages_reject_before_execution() {
	let (service, dir) = service(Config::default());
	let cwd = dir.path();
	let result = service
		.dispatch(
			"search",
			call(
				&context(cwd),
				json!({"stages": [{"kind": "matches", "pattern": "x"}]}),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("cannot start"));
}

#[tokio::test]
async fn match_set_cannot_be_grepped_without_files_of() {
	let (service, dir) = service(Config::default());
	let cwd = dir.path();
	let result = service
		.dispatch(
			"search",
			call(
				&context(cwd),
				json!({"stages": [
					{"kind": "grep", "pattern": "a", "path": "."},
					{"kind": "grep", "pattern": "b"}
				]}),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("files_of"));
}

#[tokio::test]
async fn fake_rg_receives_literal_arguments_no_shell() {
	let dir = TempDir::new().unwrap();
	let _fake = fake_rg(dir.path(), "printf '%s\\0' \"$@\"; exit 0");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let result = service
		.dispatch(
			"glob",
			call(&context(cwd), json!({"pattern": "-leading-dash *"})),
		)
		.await;
	assert_eq!(result["error"], false);
	let content: Value = result["content"].clone();
	assert_eq!(content["kind"], "paths");
	// The fake echoed its argv back as results; assert the args were passed
	// intact as separate argv entries, not interpolated.
	let items = content["items"].as_array().unwrap();
	let argv: Vec<String> = items
		.iter()
		.map(|v| v.as_str().unwrap().to_string())
		.collect();
	assert!(argv.contains(&"--files".to_string()));
	assert!(argv.contains(&"-leading-dash *".to_string()));
}

#[tokio::test]
async fn missing_rg_is_actionable_not_empty() {
	// Point at a nonexistent executable.
	let (service, dir) = service(Config {
		rg_program: "/nonexistent/rg".into(),
		..Config::default()
	});
	let cwd = dir.path();
	let result = service
		.dispatch("glob", call(&context(cwd), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("search backend unavailable"));
}

#[tokio::test]
async fn fake_rg_error_exit_is_a_failure() {
	let dir = TempDir::new().unwrap();
	let _ = fake_rg(dir.path(), "exit 3");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let result = service
		.dispatch("glob", call(&context(cwd), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("exit code 3"));
}

#[tokio::test]
async fn a_backend_failure_carries_what_rg_said() {
	let dir = TempDir::new().unwrap();
	let _ = fake_rg(
		dir.path(),
		"printf 'regex parse error:\\n    a\\\\nb\\nerror: the literal is not allowed in a regex\\n' >&2; exit 2",
	);
	let (service, _) = service(cfg_with_rg(dir.path()));
	let result = service
		.dispatch("glob", call(&context(dir.path()), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], true);
	let said = result["content"].as_str().unwrap();
	assert!(said.contains("exit code 2"), "{said}");
	assert!(said.contains("is not allowed in a regex"), "{said}");
	assert!(!said.contains('\n'), "one line: {said}");
}

#[tokio::test]
async fn pre_cancelled_invocation_spawns_no_process() {
	let dir = TempDir::new().unwrap();
	let _fake = fake_rg(dir.path(), "printf 'x\\n'; exit 0");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let ctx = context(cwd);
	// Cancel before register.
	service
		.dispatch("glob", json!({"op": "cancel", "context": ctx}))
		.await;
	let result = service
		.dispatch("glob", call(&ctx, json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("cancelled"));
}

#[tokio::test]
async fn malformed_backend_output_fails_explicitly() {
	let dir = TempDir::new().unwrap();
	fake_rg(dir.path(), "printf 'not-json\\n'; exit 0");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let result = service
		.dispatch(
			"search",
			call(
				&context(cwd),
				json!({"stages": [{"kind": "grep", "pattern": "x", "path": "."}]}),
			),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("malformed rg output"));
}

#[tokio::test]
async fn matching_json_backend_produces_typed_items() {
	let dir = TempDir::new().unwrap();
	fake_rg(
		dir.path(),
		"printf '%s\\n' '{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"a.rs\"},\"line_number\":3,\"lines\":{\"text\":\"fn hello()\\n\"}}}'; exit 0",
	);
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let result = service
		.dispatch(
			"search",
			call(
				&context(cwd),
				json!({"stages": [{"kind": "grep", "pattern": "hello", "path": "."}]}),
			),
		)
		.await;
	assert_eq!(result["error"], false);
	let content: Value = result["content"].clone();
	assert_eq!(content["kind"], "matches");
	let items = content["items"].as_array().unwrap();
	assert_eq!(items.len(), 1);
	assert_eq!(items[0]["path"], "a.rs");
	assert_eq!(items[0]["line"], 3);
	assert_eq!(items[0]["text"], "fn hello()");
}

#[tokio::test]
async fn output_survives_cancellation_polls() {
	let dir = TempDir::new().unwrap();
	fake_rg(
		dir.path(),
		"printf 'first.rs\\0'; sleep 0.02; printf 'second.rs\\0'",
	);
	let (service, _) = service(cfg_with_rg(dir.path()));
	let result = service
		.dispatch("glob", call(&context(dir.path()), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], false, "{result}");
	let content: Value = result["content"].clone();
	assert_eq!(content["items"], json!(["first.rs", "second.rs"]));
}

#[tokio::test]
async fn user_config_cannot_change_results() {
	let dir = TempDir::new().unwrap();
	// The fake records whether --no-config was in argv.
	fake_rg(dir.path(), "printf '%s\\0' \"$@\"; exit 0");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let cwd = dir.path();
	let result = service
		.dispatch("glob", call(&context(cwd), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], false);
	let content: Value = result["content"].clone();
	let argv: Vec<String> = content["items"]
		.as_array()
		.unwrap()
		.iter()
		.map(|v| v.as_str().unwrap().to_string())
		.collect();
	assert!(argv.contains(&"--no-config".to_string()));
}

#[tokio::test]
async fn deadlock_deadline_terminates_and_reaps_child() {
	let dir = TempDir::new().unwrap();
	// A child that prints one line then sleeps forever: the deadline must kill
	// and reap it rather than hang the call.
	fake_rg(dir.path(), "printf 'slow\\n'; exec sleep 30");
	let cwd = dir.path();
	let ctx = context(cwd);
	{
		let c = Config {
			rg_program: dir.path().join("rg").to_string_lossy().into_owned(),
			max_search_wait_ms: 200,
			..Config::default()
		};
		let (service, _) = service(c);
		let result = service
			.dispatch("glob", call(&ctx, json!({"pattern": "*"})))
			.await;
		assert_eq!(result["error"], true);
		assert!(result["content"].as_str().unwrap().contains("deadline"));
	}
	// The deadline path killed and waited on the direct child (`exec` made it
	// the sleep), so the reap is proven by dispatch returning; no pgrep — a
	// parallel test may legitimately hold its own child process.
}

#[tokio::test]
async fn oversized_results_report_truncation() {
	let dir = TempDir::new().unwrap();
	fake_rg(dir.path(), "printf '%s\\0' a b c d e f g; exit 0");
	let c = Config {
		rg_program: dir.path().join("rg").to_string_lossy().into_owned(),
		max_search_items: 4,
		..Config::default()
	};
	let (service, _) = service(c);
	let cwd = dir.path();
	let result = service
		.dispatch("glob", call(&context(cwd), json!({"pattern": "*"})))
		.await;
	assert_eq!(result["error"], false);
	let content: Value = result["content"].clone();
	assert_eq!(content["items"].as_array().unwrap().len(), 4);
	assert_eq!(content["truncated"], true);
	assert_eq!(content["complete"], false);
}

#[tokio::test]
async fn page_has_a_continuation_for_the_rest() {
	let (service, dir) = service(Config {
		max_search_items: 2,
		..Config::default()
	});
	for i in 0..7 {
		std::fs::write(dir.path().join(format!("file-{i}")), "x").unwrap();
	}
	let result = service
		.dispatch("glob", call(&context(dir.path()), json!({"pattern":"*"})))
		.await;
	assert_eq!(result["error"], false, "{result}");
	let content: Value = result["content"].clone();
	assert!(
		content["next_cursor"].is_string(),
		"truncated results need continuation: {content}"
	);
}

async fn response(service: &Arc<Service>, cwd: &Path, input: Value) -> Value {
	let result = service.dispatch("search", call(&context(cwd), input)).await;
	assert_eq!(result["error"], false, "{result}");
	result["content"].clone()
}

#[tokio::test]
async fn large_real_tree_pages_every_identity_once_and_ref_keeps_the_tail() {
	let (service, dir) = service(Config {
		max_search_items: 37,
		max_search_bytes: 1024,
		..Config::default()
	});
	let mut expected: Vec<String> = (0..1100)
		.map(|i| format!("file-{i:04}-long-name.txt"))
		.collect();
	expected.push("newline\nname.txt".into());
	for name in &expected {
		std::fs::write(dir.path().join(name), "hit").unwrap();
	}
	expected.sort();
	let first = response(&service, dir.path(), json!({"stages":[{"kind":"files"}]})).await;
	assert_eq!(first["total_count"], expected.len());
	let mut page = first.clone();
	let mut got = Vec::new();
	loop {
		assert_eq!(page["results_revision"], first["results_revision"]);
		assert_eq!(page["consistency"], "captured_results");
		let items = page["items"].as_array().unwrap();
		assert!(items.len() <= 37);
		assert!(page["items"].to_string().len() <= 1024);
		got.extend(items.iter().map(|v| v.as_str().unwrap().to_owned()));
		if page["next_cursor"].is_null() {
			assert_eq!(page["complete"], true);
			break;
		}
		assert_eq!(page["complete"], false);
		page = response(&service, dir.path(), json!({"cursor":page["next_cursor"]})).await;
	}
	assert_eq!(got, expected);
	let tail = response(
		&service,
		dir.path(),
		json!({"ref":first["ref"],"stages":[{"kind":"files","pattern":"file-1099*"}]}),
	)
	.await;
	assert_eq!(tail["items"], json!(["file-1099-long-name.txt"]));
}

#[tokio::test]
async fn captured_pages_replay_after_tree_changes_and_new_queries_refresh() {
	let (service, dir) = service(Config {
		max_search_items: 2,
		..Config::default()
	});
	for name in ["a", "b", "c"] {
		std::fs::write(dir.path().join(name), "hit").unwrap();
	}
	let first = response(&service, dir.path(), json!({"stages":[{"kind":"files"}]})).await;
	let old = response(&service, dir.path(), json!({"cursor":first["next_cursor"]})).await;
	std::fs::remove_file(dir.path().join("c")).unwrap();
	std::fs::write(dir.path().join("d"), "new").unwrap();
	let replay = response(&service, dir.path(), json!({"cursor":first["next_cursor"]})).await;
	assert_eq!(replay, old);
	assert_eq!(replay["items"], json!(["c"]));
	assert!(replay["tree_changes"]
		.as_str()
		.unwrap()
		.contains("not_revalidated"));
	let fresh = response(&service, dir.path(), json!({"stages":[{"kind":"files"}]})).await;
	let fresh_tail = response(&service, dir.path(), json!({"cursor":fresh["next_cursor"]})).await;
	assert_eq!(fresh_tail["items"], json!(["d"]));
	let exact = response(
		&service,
		dir.path(),
		json!({"stages":[{"kind":"files","pattern":"[ab]"}]}),
	)
	.await;
	assert_eq!(exact["items"], json!(["a", "b"]));
	assert_eq!(exact["complete"], true);
	assert!(exact["next_cursor"].is_null());
}

#[tokio::test]
async fn cursor_scope_expiry_and_cancellation_are_explicit() {
	let (service, dir) = service(Config {
		max_search_items: 1,
		..Config::default()
	});
	for name in ["a", "b"] {
		std::fs::write(dir.path().join(name), "x").unwrap();
	}
	let first = response(&service, dir.path(), json!({"stages":[{"kind":"files"}]})).await;
	let cursor = first["next_cursor"].as_str().unwrap();
	for input in [
		json!({"cursor":"garbage"}),
		json!({"cursor":format!("{cursor}x")}),
		json!({"cursor":cursor,"stages":[{"kind":"files"}]}),
	] {
		let result = service
			.dispatch("search", call(&context(dir.path()), input))
			.await;
		assert_eq!(result["error"], true);
		assert!(
			result["content"]
				.as_str()
				.unwrap()
				.contains("invalid_cursor"),
			"{result}"
		);
	}
	let foreign = Context {
		session: "foreign".into(),
		..context(dir.path())
	};
	let result = service
		.dispatch("search", call(&foreign, json!({"cursor":cursor})))
		.await;
	assert_eq!(result["error"], true);
	let other_dir = TempDir::new().unwrap();
	let result = service
		.dispatch(
			"search",
			call(&context(other_dir.path()), json!({"cursor":cursor})),
		)
		.await;
	assert_eq!(result["error"], true);
	let cancelled = context(dir.path());
	service
		.dispatch("search", json!({"op":"cancel","context":cancelled}))
		.await;
	let result = service
		.dispatch("search", call(&cancelled, json!({"cursor":cursor})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("cancelled"));
	let newer = Context {
		run: "r2".into(),
		..context(dir.path())
	};
	let result = service
		.dispatch("search", call(&newer, json!({"stages":[]})))
		.await;
	assert_eq!(result["error"], false);
	let result = service
		.dispatch(
			"search",
			call(&context(dir.path()), json!({"cursor":cursor})),
		)
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("run_disposed"));
}

#[tokio::test]
async fn real_grep_keeps_over_200_matches_and_empty_chains_do_not_sweep() {
	let (service, dir) = service(Config {
		max_search_items: 73,
		..Config::default()
	});
	std::fs::write(dir.path().join("many.txt"), "hit\n".repeat(301)).unwrap();
	let mut page = response(
		&service,
		dir.path(),
		json!({"stages":[{"kind":"grep","pattern":"hit"}]}),
	)
	.await;
	let mut lines = Vec::new();
	loop {
		for item in page["items"].as_array().unwrap() {
			assert_eq!(item["path"], "many.txt");
			lines.push(item["line"].as_u64().unwrap());
		}
		if page["next_cursor"].is_null() {
			break;
		}
		page = response(&service, dir.path(), json!({"cursor":page["next_cursor"]})).await;
	}
	assert_eq!(lines, (1..=301).collect::<Vec<_>>());
	for stage in [
		json!({"kind":"files"}),
		json!({"kind":"grep","pattern":"hit"}),
	] {
		let empty = response(
			&service,
			dir.path(),
			json!({"stages":[{"kind":"files","pattern":"absent-*"},stage]}),
		)
		.await;
		assert_eq!(empty["count"], 0);
		assert_eq!(empty["complete"], true);
	}
	let projected = response(
		&service,
		dir.path(),
		json!({"stages":[{"kind":"grep","pattern":"hit"},{"kind":"files_of"}]}),
	)
	.await;
	assert_eq!(projected["items"], json!(["many.txt"]));
}

#[tokio::test]
async fn bounded_backend_reads_preserve_the_limit_and_reject_one_extra_byte() {
	for limit in [0, 1, 1024, 65536] {
		let exact = vec![b'x'; limit];
		assert_eq!(read_bounded(exact.as_slice(), limit).await.unwrap(), exact);
		let oversized = vec![b'x'; limit + 1];
		let (first, second) = oversized.split_at(limit / 2);
		let error = read_bounded(tokio::io::AsyncReadExt::chain(first, second), limit)
			.await
			.unwrap_err();
		assert!(error.contains("capacity_exceeded"), "{error}");
	}
}

#[tokio::test]
async fn bounded_backend_overflow_does_not_wait_for_eof() {
	let (mut writer, reader) = tokio::io::duplex(8);
	tokio::io::AsyncWriteExt::write_all(&mut writer, b"1234567")
		.await
		.unwrap();
	let error = tokio::time::timeout(Duration::from_secs(5), read_bounded(reader, 6))
		.await
		.expect("overflow must be detected while the writer is still open")
		.unwrap_err();
	assert!(error.contains("capacity_exceeded"), "{error}");
}

#[tokio::test]
async fn floods_and_oversized_items_fail_instead_of_claiming_completeness() {
	// Exercise the byte cap, not thousands of tiny shell writes racing the deadline.
	for body in [
		"while :; do printf '%65536s' x; done",
		"while :; do printf '%65536s' x >&2; done",
	] {
		let dir = TempDir::new().unwrap();
		fake_rg(dir.path(), body);
		let (service, _) = service(Config {
			max_search_backend_bytes: 1024,
			max_search_wait_ms: 2000,
			..cfg_with_rg(dir.path())
		});
		let result = service
			.dispatch("glob", call(&context(dir.path()), json!({})))
			.await;
		assert_eq!(result["error"], true);
		assert!(
			result["content"]
				.as_str()
				.unwrap()
				.contains("capacity_exceeded"),
			"{result}"
		);
	}
	let (service, dir) = service(Config {
		max_search_bytes: 1024,
		..Config::default()
	});
	std::fs::write(dir.path().join("big.txt"), "hit".repeat(1024)).unwrap();
	let result = service
		.dispatch("grep", call(&context(dir.path()), json!({"pattern":"hit"})))
		.await;
	assert_eq!(result["error"], true);
	assert!(
		result["content"]
			.as_str()
			.unwrap()
			.contains("one search item"),
		"{result}"
	);
}

#[tokio::test]
async fn results_budget_fails_without_eviction_and_retirement_frees_it() {
	let (service, dir) = service(Config {
		max_search_items: 1,
		max_search_results_bytes: 40,
		..Config::default()
	});
	for name in ["alpha", "bravo", "charlie"] {
		std::fs::write(dir.path().join(name), "x").unwrap();
	}
	let first = response(&service, dir.path(), json!({"stages":[{"kind":"files"}]})).await;
	let result = service
		.dispatch("glob", call(&context(dir.path()), json!({})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"]
		.as_str()
		.unwrap()
		.contains("search results"));
	let page = response(&service, dir.path(), json!({"cursor":first["next_cursor"]})).await;
	assert_eq!(page["items"], json!(["bravo"]));
	let ctx = Context {
		run: "new".into(),
		..context(dir.path())
	};
	let result = service.dispatch("glob", call(&ctx, json!({}))).await;
	assert_eq!(result["error"], false, "{result}");
}

#[tokio::test]
async fn deadline_covers_child_wait_after_stdout_is_closed() {
	let dir = TempDir::new().unwrap();
	fake_rg(dir.path(), "exec 1>&-; exec sleep 30");
	let (service, _) = service(Config {
		max_search_wait_ms: 200,
		..cfg_with_rg(dir.path())
	});
	let result = service
		.dispatch("glob", call(&context(dir.path()), json!({})))
		.await;
	assert_eq!(result["error"], true);
	assert!(result["content"].as_str().unwrap().contains("deadline"));
}

#[tokio::test]
async fn active_search_cancellation_reaps_the_backend() {
	let dir = TempDir::new().unwrap();
	fake_rg(dir.path(), "printf ready > ready; exec sleep 30");
	let (service, _) = service(cfg_with_rg(dir.path()));
	let ctx = context(dir.path());
	let worker_service = service.clone();
	let worker_context = ctx.clone();
	let worker = tokio::spawn(async move {
		worker_service
			.dispatch("glob", call(&worker_context, json!({})))
			.await
	});
	tokio::time::timeout(Duration::from_secs(5), async {
		while !dir.path().join("ready").exists() {
			tokio::time::sleep(Duration::from_millis(1)).await;
		}
	})
	.await
	.unwrap();
	service
		.dispatch("glob", json!({"op":"cancel","context":ctx}))
		.await;
	let result = tokio::time::timeout(Duration::from_secs(5), worker)
		.await
		.unwrap()
		.unwrap();
	assert_eq!(result["error"], true);
	assert!(
		result["content"].as_str().unwrap().contains("cancelled"),
		"{result}"
	);
}

#[tokio::test]
async fn a_dot_directory_is_searched_and_the_git_store_is_not() {
	// This workspace keeps its memo record, its hooks, its tests and its
	// harness settings under dot-directories. A search that cannot reach them
	// answers a false empty, which reads exactly like a true one. `.git/` is
	// the one place still excluded: with submodules nested here its object
	// store would outnumber every real result.
	let (service, dir) = service(Config::default());
	let cwd = dir.path();
	for (path, text) in [
		(".kern/hooks/request-assessment.ts", "the marker\n"),
		(".claude/settings.json", "{\"marker\": true}\n"),
		(".git/objects/loose", "the marker\n"),
		("plain.txt", "nothing here\n"),
	] {
		let path = cwd.join(path);
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(path, text).unwrap();
	}

	let listed = service
		.dispatch("glob", call(&context(cwd), json!({"pattern": "*"})))
		.await;
	assert_eq!(listed["error"], false, "{listed}");
	let listed: Value = listed["content"].clone();
	let paths: Vec<&str> = listed["items"]
		.as_array()
		.unwrap()
		.iter()
		.map(|item| item.as_str().unwrap())
		.collect();
	assert!(
		paths.contains(&".kern/hooks/request-assessment.ts"),
		"{paths:?}"
	);
	assert!(paths.contains(&".claude/settings.json"), "{paths:?}");
	assert!(
		!paths.iter().any(|path| path.starts_with(".git/")),
		"{paths:?}"
	);

	let found = service
		.dispatch("grep", call(&context(cwd), json!({"pattern": "marker"})))
		.await;
	assert_eq!(found["error"], false, "{found}");
	let found: Value = found["content"].clone();
	let hits: Vec<&str> = found["items"]
		.as_array()
		.unwrap()
		.iter()
		.map(|item| item["path"].as_str().unwrap())
		.collect();
	assert!(
		hits.contains(&".kern/hooks/request-assessment.ts"),
		"{hits:?}"
	);
	assert!(hits.contains(&".claude/settings.json"), "{hits:?}");
	assert!(
		!hits.iter().any(|path| path.starts_with(".git/")),
		"{hits:?}"
	);
}

#[tokio::test]
async fn every_backend_call_carries_the_shared_flags() {
	// One place decides what `rg` is asked for. A second spawn site that
	// forgot `--hidden` would answer half the workspace, and the two halves of
	// one search would disagree about what exists.
	let (_service, dir) = service(Config::default());
	assert_eq!(
		super::base_argv(&[]),
		["--no-config", "--no-follow", "--hidden", "--glob=!.git/"]
	);
	// rg lets the last matching glob decide, so the caller's glob is placed
	// before the exclusion. Reversed, a caller asking for `*` re-includes the
	// object store, which is exactly how this was first written.
	assert_eq!(
		super::base_argv(&["*.rs"]),
		[
			"--no-config",
			"--no-follow",
			"--hidden",
			"--glob",
			"*.rs",
			"--glob=!.git/"
		]
	);
	drop(dir);
}
