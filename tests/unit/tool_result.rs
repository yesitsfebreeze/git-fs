use super::*;

#[test]
fn payloads_are_encoded_once_and_text_stays_literal() {
	let text = r#"{"literal":true}"#;
	assert_eq!(success(json!(text)).unwrap()["content"], text);
	let result = success(json!({"content":text,"found":true})).unwrap();
	assert_eq!(
		result["content"].clone(),
		json!({"content":text,"found":true})
	);
	assert_eq!(failure("ordinary failure".into())["error"], true);
}

#[tokio::test]
async fn invalid_inputs_do_not_open_the_store() {
	let dir = tempfile::tempdir().unwrap();
	assert!(std::process::Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	let config = std::sync::Arc::new(crate::store::StoreConfig::new(".kern/gitfs"));
	for input in [
		json!(null),
		json!({"op":"write","path":"a","content":"x"}),
		json!({"op":"rollback"}),
		json!({"op":"ls","session":7}),
		json!({"op":"diff","path":"../escape"}),
		json!({"op":"capture","paths":[7]}),
		json!({"op":"ls","unexpected":true}),
	] {
		assert!(crate::overlay::handle(config.clone(), json!({"op":"call","context":{"session":"test","run":"r","call":"1","cwd":dir.path()},"input":input})).await.is_err());
		assert!(!dir.path().join(".kern").exists());
	}
	for input in [
		json!({"op":"scan","paths":["../escape"]}),
		json!({"op":"integrate","force_secrets":"yes"}),
		json!({"op":"undo","paths":[]}),
	] {
		assert!(validate("integrate", &input).is_err());
	}
}

#[test]
fn inspection_baseline_diff_is_available() {
	assert!(
		validate("gitfs", &json!({"op":"diff","path":"a"})).is_ok(),
		"diff must be a recognized read-only operation"
	);
	// A small page is a page, not a refusal (FAILURES 2026-09-29 21:22).
	assert!(validate("gitfs", &json!({"op":"diff","path":"a","limit":40})).is_ok());
	assert!(validate("gitfs", &json!({"op":"diff","path":"a","limit":0})).is_err());
	let schema = crate::overlay::describe();
	assert_eq!(schema["input_schema"]["properties"]["limit"]["minimum"], 1);
}

#[tokio::test]
async fn inspection_baseline_ls_does_not_create_a_store() {
	let dir = tempfile::TempDir::new().unwrap();
	assert!(std::process::Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	std::fs::write(dir.path().join("a"), "disk").unwrap();
	let cfg = std::sync::Arc::new(crate::store::StoreConfig::new(".kern/gitfs"));
	let result = crate::overlay::handle(
		cfg,
		json!({"op":"call","context":{"cwd":dir.path(),"session":"inspect"},"input":{"op":"ls"}}),
	)
	.await
	.unwrap();
	assert_eq!(result["error"], false);
	assert!(
		!dir.path().join(".kern").exists(),
		"read-only inspection created a store"
	);
}

#[tokio::test]
async fn inspection_absence_binary_bounds_and_path_failures_are_explicit() {
	use std::sync::Arc;
	let dir = tempfile::TempDir::new().unwrap();
	assert!(std::process::Command::new("git")
		.args(["init", "-q"])
		.current_dir(dir.path())
		.status()
		.unwrap()
		.success());
	let cfg = Arc::new(crate::store::StoreConfig::new(".kern/gitfs"));
	let request =
		|input| json!({"op":"call","context":{"cwd":dir.path(),"session":"inspect"},"input":input});
	for input in [json!({"op":"ls"}), json!({"op":"diff","path":"absent"})] {
		let result = crate::overlay::handle(cfg.clone(), request(input))
			.await
			.unwrap();
		assert_eq!(result["error"], false);
	}
	assert!(!dir.path().join(".kern").exists());
	std::fs::write(dir.path().join("binary"), [0, 255, 1]).unwrap();
	let binary = crate::overlay::handle(cfg.clone(), request(json!({"op":"diff","path":"binary"})))
		.await
		.unwrap();
	let content: serde_json::Value = binary["content"].clone();
	assert_eq!(content["disk"]["bytes"], 3);
	assert!(content["patch"]
		.as_str()
		.unwrap()
		.contains("Binary content differs"));
	assert_eq!(content["base"]["revision"], serde_json::Value::Null);
	assert_eq!(content["conflict"], false);
	// Sparse, so the cap in force costs nothing to exceed.
	let cap = crate::limits::limits().source_bytes;
	let large = std::fs::File::create(dir.path().join("large")).unwrap();
	large.set_len(cap + 1).unwrap();
	let result =
		crate::overlay::handle(cfg.clone(), request(json!({"op":"diff","path":"large"}))).await;
	let refusal = result.unwrap_err();
	assert!(refusal.contains("inspection cap"), "{refusal}");
	assert!(refusal.contains(&cap.to_string()), "{refusal}");
	for input in [
		json!({"op":"diff","path":"binary","limit":0}),
		json!({"op":"diff","path":"binary","offset":1}),
		json!({"op":"diff","path":"../escape"}),
		json!({"op":"diff","path":"binary","expected_revision":"stale"}),
	] {
		assert!(crate::overlay::handle(cfg.clone(), request(input))
			.await
			.is_err());
	}
	#[cfg(unix)]
	{
		let outside = tempfile::TempDir::new().unwrap();
		std::fs::write(outside.path().join("secret"), "private").unwrap();
		std::os::unix::fs::symlink(outside.path().join("secret"), dir.path().join("link")).unwrap();
		assert!(
			crate::overlay::handle(cfg.clone(), request(json!({"op":"diff","path":"link"})))
				.await
				.unwrap_err()
				.contains("escapes")
		);
	}
	assert!(!dir.path().join(".kern").exists());
}

#[test]
fn a_bare_push_passes_validation_and_a_mistyped_field_still_fails() {
	validate("integrate", &json!({"op":"push"})).unwrap();
	validate("integrate", &json!({"op":"push","remote":"origin"})).unwrap();
	assert!(validate("integrate", &json!({"op":"push","commit":7})).is_err());
}

#[test]
fn an_unknown_field_is_answered_with_the_fields_the_op_takes() {
	let error = validate("integrate", &json!({"op":"integrate","subject":"x"})).unwrap_err();
	assert!(
		error.contains("subject is not valid for integrate integrate; integrate takes message, force_secrets, phase, expected_revision"),
		"{error}"
	);
	let error = validate("integrate", &json!({"op":"undo","paths":[]})).unwrap_err();
	assert!(error.ends_with("undo takes no fields"), "{error}");
}

#[test]
fn a_session_in_input_is_pointed_to_context() {
	let error = validate("gitfs", &json!({"op":"catchup","session":"a"})).unwrap_err();
	assert!(
		error.contains("session goes in `context`, not `input`"),
		"{error}"
	);
	assert!(error.ends_with("catchup takes no fields"), "{error}");
	let error = validate("integrate", &json!({"op":"undo","session":"a"})).unwrap_err();
	assert!(error.contains("session goes in `context`"), "{error}");
	validate("gitfs", &json!({"op":"ls","session":"a"})).unwrap();
}
