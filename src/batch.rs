//! Explicit independent read batches. The MCP transport remains sequential,
//! so a following mutation cannot overtake the reads that establish freshness.
use crate::service::Context;
use futures::{stream, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};

const MAX_REQUESTS: usize = 32;
const MAX_CONCURRENCY: usize = 8;
const READ_TOOLS: [&str; 4] = ["read", "glob", "grep", "search"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
	tool: String,
	arguments: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
	requests: Vec<Request>,
	#[serde(default = "default_concurrency")]
	concurrency: usize,
}

fn default_concurrency() -> usize {
	4
}

pub fn describe() -> Value {
	json!({
		"name": "batch", "reads": true,
		"description": "Run independent read, glob, grep or search requests concurrently in one call. Results stay in input order, each with its own error and revision. Reads establish freshness for later edits. No writes or nested batches. Search refs/cursors must come from an earlier completed call; batch entries cannot depend on each other. All entries inherit the caller's workspace and session (a read may select a peer session). Not an atomic snapshot.",
		"input_schema": {
			"type": "object", "additionalProperties": false, "required": ["requests"],
			"properties": {
				"requests": {"type": "array", "minItems": 1, "maxItems": MAX_REQUESTS,
					"items": {"type": "object", "additionalProperties": false,
						"required": ["tool", "arguments"], "properties": {
							"tool": {"type": "string", "enum": READ_TOOLS},
							"arguments": {"type": "object", "description": "Arguments of the selected tool; omit host routing/context fields."}
						}}},
				"concurrency": {"type": "integer", "minimum": 1, "maximum": MAX_CONCURRENCY, "default": default_concurrency()}
			}
		}
	})
}

fn parse(input: Value) -> Result<Input, String> {
	let input: Input = serde_json::from_value(input).map_err(|e| format!("invalid batch: {e}"))?;
	if input.requests.is_empty() || input.requests.len() > MAX_REQUESTS {
		return Err(format!("batch requires 1..={MAX_REQUESTS} requests"));
	}
	if !(1..=MAX_CONCURRENCY).contains(&input.concurrency) {
		return Err(format!("batch concurrency must be 1..={MAX_CONCURRENCY}"));
	}
	// Validate the entire envelope before starting any child: mutations, nested
	// batches and per-child routing must never slip through a shared hook call.
	for (index, request) in input.requests.iter().enumerate() {
		if !READ_TOOLS.contains(&request.tool.as_str()) {
			return Err(format!(
				"batch request {index}: only read, glob, grep and search are allowed"
			));
		}
		if ["_git_fs", "context", "cwd", "run", "call"]
			.iter()
			.any(|key| request.arguments.contains_key(*key))
		{
			return Err(format!(
				"batch request {index}: routing context is inherited from the caller"
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

pub async fn run(input: Value, context: &Context) -> Result<Value, String> {
	context.validate()?;
	let input = parse(input)?;
	let jobs = input
		.requests
		.into_iter()
		.enumerate()
		.map(|(index, request)| {
			let mut child = context.clone();
			child.call = format!("{}:batch:{index}", context.call);
			async move {
				let answer = match child.validate() {
					Ok(()) => {
						crate::call_single(&request.tool, Value::Object(request.arguments), &child)
							.await
					}
					Err(error) => Err(error),
				}
				.unwrap_or_else(|error| json!({"content": error, "error": true}));
				json!({"index": index, "tool": request.tool, "result": answer})
			}
		});
	let results = collect(jobs, input.concurrency).await;
	let failed = results
		.iter()
		.filter(|r| r["result"]["error"] == true)
		.count();
	Ok(json!({"content": {"results": results, "failed": failed}, "error": failed > 0}))
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
	fn refuses_mutations_recursion_context_overrides_and_invalid_bounds() {
		for tool in ["write", "edit", "integrate", "batch", "unknown"] {
			assert!(parse(json!({"requests":[{"tool":tool,"arguments":{}}]})).is_err());
		}
		for args in [json!({"_git_fs":{}}), json!({"cwd":"/elsewhere"})] {
			assert!(parse(json!({"requests":[{"tool":"read","arguments":args}]})).is_err());
		}
		for concurrency in [0, 9] {
			assert!(parse(
				json!({"requests":[{"tool":"glob","arguments":{}}],"concurrency":concurrency})
			)
			.is_err());
		}
		for n in [0, 33] {
			assert!(
				parse(json!({"requests":vec![json!({"tool":"glob","arguments":{}}); n]})).is_err()
			);
		}
	}
}
