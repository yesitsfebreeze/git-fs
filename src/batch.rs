//! Per-tool multiple-item calls. Read-only file operations may run concurrently;
//! mutations run in order and stop after the first failure.
use crate::service::Context;
use futures::{stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};

const MAX_ITEMS: usize = 32;
const MAX_CONCURRENCY: usize = 8;

fn parallel(tool: &str) -> bool {
	matches!(tool, "read" | "glob" | "grep" | "search")
}

/// Keep the ordinary schema at the root for host compatibility, and describe
/// the mutually exclusive single-item and multiple-item forms.
pub fn describe(tool: &str, mut description: Value) -> Value {
	let item = description["input_schema"].clone();
	let mut properties = item["properties"].as_object().cloned().unwrap_or_default();
	let single_keys: Vec<Value> = properties
		.keys()
		.map(|key| json!({"required": [key]}))
		.collect();
	properties.insert("items".into(), json!({
		"type": "array", "minItems": 1, "maxItems": MAX_ITEMS, "items": item,
		"description": "One or more argument objects for this tool. Do not mix with single-item fields or include routing context."
	}));
	if parallel(tool) {
		properties.insert(
			"concurrency".into(),
			json!({
				"type": "integer", "minimum": 1, "maximum": MAX_CONCURRENCY, "default": 4
			}),
		);
	}
	let mut single = item.clone();
	let single_object = single.as_object_mut().expect("tool input schema");
	single_object.remove("properties");
	single_object.remove("additionalProperties");
	single_object.insert(
		"not".into(),
		json!({"anyOf": [
			{"required": ["items"]}, {"required": ["concurrency"]}
		]}),
	);
	let mut multiple = json!({"required": ["items"]});
	if !single_keys.is_empty() {
		multiple["not"] = json!({"anyOf": single_keys});
	}
	description["input_schema"] = json!({
		"type": "object", "properties": properties, "additionalProperties": false,
		"oneOf": [single, multiple]
	});
	let behavior = if parallel(tool) {
		"Items run concurrently (default 4, maximum 8); results stay in input order."
	} else {
		"Items run sequentially and stop on the first error; remaining items are reported as skipped."
	};
	description["description"] = json!(format!(
		"{} Accepts ordinary single-item arguments or {{items: [arguments, ...]}} (1-32 items). {behavior} Each result retains its error and revision. Not atomic; completed changes are not rolled back. All items inherit the caller's workspace and session.",
		description["description"].as_str().unwrap_or_default()
	));
	description
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
	items: Vec<serde_json::Map<String, Value>>,
	concurrency: Option<usize>,
}

fn parse(tool: &str, input: Value) -> Result<Input, String> {
	let input: Input = serde_json::from_value(input).map_err(|e| format!("invalid items: {e}"))?;
	if input.items.is_empty() || input.items.len() > MAX_ITEMS {
		return Err(format!("items requires 1..={MAX_ITEMS} argument objects"));
	}
	if let Some(concurrency) = input.concurrency {
		if !parallel(tool) {
			return Err("concurrency is only supported by read, glob, grep and search".into());
		}
		if !(1..=MAX_CONCURRENCY).contains(&concurrency) {
			return Err(format!("concurrency must be 1..={MAX_CONCURRENCY}"));
		}
	}
	// Reject invalid envelopes before any item can change state.
	for (index, arguments) in input.items.iter().enumerate() {
		if [
			"items",
			"concurrency",
			"_git_fs",
			"context",
			"cwd",
			"run",
			"call",
		]
		.iter()
		.any(|key| arguments.contains_key(*key))
		{
			return Err(format!(
				"item {index}: nested items and routing overrides are not allowed"
			));
		}
	}
	Ok(input)
}

async fn collect<F: std::future::Future<Output = Value>>(
	jobs: impl IntoIterator<Item = F>,
	concurrency: usize,
) -> Vec<Value> {
	stream::iter(jobs).buffered(concurrency).collect().await
}

pub async fn run(tool: &str, input: Value, context: &Context) -> Result<Value, String> {
	context.validate()?;
	let input = parse(tool, input)?;
	// Validate every child context up front, including the bounded call ID.
	let mut children = Vec::with_capacity(input.items.len());
	for index in 0..input.items.len() {
		let mut child = context.clone();
		child.call = format!("{}:items:{index}", context.call);
		child.validate()?;
		children.push(child);
	}
	let jobs = input.items.into_iter().zip(children).enumerate().map(
		|(index, (arguments, child))| async move {
			let result = crate::call_single(tool, Value::Object(arguments), &child)
				.await
				.unwrap_or_else(crate::tool_result::failure);
			json!({"index": index, "result": result})
		},
	);
	let results = if parallel(tool) {
		collect(jobs, input.concurrency.unwrap_or(4)).await
	} else {
		let mut stopped = false;
		let mut results = Vec::new();
		for (index, job) in jobs.enumerate() {
			if stopped {
				results.push(json!({"index": index, "skipped": true,
					"result": {"error": true, "content": "skipped after an earlier item failed"}}));
			} else {
				let result = job.await;
				stopped = result["result"]["error"] == true;
				results.push(result);
			}
		}
		results
	};
	let failed = results
		.iter()
		.filter(|r| r["skipped"] != true && r["result"]["error"] == true)
		.count();
	let skipped = results.iter().filter(|r| r["skipped"] == true).count();
	Ok(
		json!({"content": {"results": results, "failed": failed, "skipped": skipped}, "error": failed > 0}),
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::{
		atomic::{AtomicUsize, Ordering},
		Arc,
	};

	#[tokio::test]
	async fn concurrent_bounded_and_ordered_even_when_completion_is_reversed() {
		let active = Arc::new(AtomicUsize::new(0));
		let peak = Arc::new(AtomicUsize::new(0));
		let barrier = Arc::new(tokio::sync::Barrier::new(2));
		let jobs = (0..6).map(|i| {
			let (active, peak, barrier) = (active.clone(), peak.clone(), barrier.clone());
			async move {
				let n = active.fetch_add(1, Ordering::SeqCst) + 1;
				peak.fetch_max(n, Ordering::SeqCst);
				barrier.wait().await;
				if i % 2 == 0 {
					tokio::time::sleep(std::time::Duration::from_millis(5)).await;
				}
				active.fetch_sub(1, Ordering::SeqCst);
				json!(i)
			}
		});
		let results = tokio::time::timeout(std::time::Duration::from_secs(5), collect(jobs, 2))
			.await
			.unwrap();
		assert_eq!(results, (0..6).map(|i| json!(i)).collect::<Vec<_>>());
		assert_eq!(peak.load(Ordering::SeqCst), 2);
	}

	#[test]
	fn rejects_invalid_envelopes_before_execution() {
		for value in [
			json!({"items": []}),
			json!({"items": vec![json!({}); 33]}),
			json!({"items": [null]}),
			json!({"items": [{"path": "a"}], "path": "b"}),
			json!({"items": [{}], "concurrency": 0}),
			json!({"items": [{}], "concurrency": 9}),
		] {
			assert!(parse("read", value).is_err());
		}
		for key in [
			"items",
			"concurrency",
			"_git_fs",
			"context",
			"cwd",
			"run",
			"call",
		] {
			assert!(parse("write", json!({"items": [{(key): {}}]})).is_err());
		}
		assert!(parse("edit", json!({"items": [{}], "concurrency": 1})).is_err());
	}

	#[test]
	fn every_tool_describes_both_forms_and_no_batch_tool_remains() {
		assert!(!crate::TOOLS.contains(&"batch"));
		assert!(crate::describe("batch").is_err());
		for tool in crate::TOOLS {
			let description = crate::describe(tool).unwrap();
			let schema = &description["input_schema"];
			assert_eq!(schema["oneOf"].as_array().unwrap().len(), 2, "{tool}");
			assert_eq!(schema["properties"]["items"]["maxItems"], MAX_ITEMS);
			assert_eq!(schema["properties"]["items"]["items"]["type"], "object");
			assert_eq!(
				schema["properties"].get("concurrency").is_some(),
				parallel(tool)
			);
		}
	}
}
