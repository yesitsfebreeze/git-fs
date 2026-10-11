//! The defaults `kern.md` declares, and configuration laid over them.
//! The same file in every Rust kern; each uses its own subset.
#![allow(dead_code)]

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Lay `over` on top of `base`, field by field; `null` removes a key.
pub fn merge(base: &mut Value, over: Value) {
	match (base, over) {
		(Value::Object(base), Value::Object(over)) => {
			for (key, value) in over {
				match value {
					Value::Null => {
						base.remove(&key);
					}
					value => match base.get_mut(&key) {
						Some(slot) => merge(slot, value),
						None => {
							base.insert(key, value);
						}
					},
				}
			}
		}
		(base, over) => *base = over,
	}
}

fn set(value: &mut Value, key: &str, leaf: Value) {
	let mut at = value;
	let mut steps = key.split('.').peekable();
	while let Some(step) = steps.next() {
		if !at.is_object() {
			*at = json!({});
		}
		let map = at.as_object_mut().expect("table on the way down");
		if steps.peek().is_none() {
			map.insert(step.to_owned(), leaf);
			return;
		}
		at = map.entry(step.to_owned()).or_insert_with(|| json!({}));
	}
}

/// Every declared default as the table a configuration lays itself over. A key
/// inside an optional table whose default is absent stays absent.
pub fn declared(document: &str) -> Value {
	#[derive(serde::Deserialize)]
	struct Document {
		#[serde(default)]
		config: BTreeMap<String, Value>,
	}
	let document: Document =
		serde_json::from_str(document).expect("a kern's own kern.md frontmatter");
	let specs = document.config;
	let mut out = json!({});
	for (key, spec) in &specs {
		let absent = key.match_indices('.').any(|(at, _)| {
			specs
				.get(&key[..at])
				.is_some_and(|outer| outer["optional"] == true && outer["default"].is_null())
		});
		if !absent {
			set(&mut out, key, spec["default"].clone());
		}
	}
	out
}
