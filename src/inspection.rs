//! Read-only comparison; all Git object reads use pinned commits.
use crate::{store::Store, tool_result::success, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

// A cap, not a constant: declared in `kern.md` under `limits`.
fn max_source() -> u64 {
	crate::limits::limits().source_bytes
}

/// The refusal, naming the cap actually in force. The number is a setting
/// (`limits.source_bytes`), so spelling it into the message is how a caller
/// told "too large" learns what large means here.
fn too_large() -> String {
	format!(
		"diff source exceeds the {}-byte inspection cap",
		max_source()
	)
}

#[derive(Clone)]
struct Side {
	bytes: Vec<u8>,
	mode: String,
}

fn object(store: &Store, commit: Option<&str>, path: &str) -> Result<Option<Side>> {
	let Some(commit) = commit else {
		return Ok(None);
	};
	let row = store.git(&["--literal-pathspecs", "ls-tree", "-z", commit, "--", path])?;
	if row.is_empty() {
		return Ok(None);
	}
	let fields = std::str::from_utf8(&row)
		.map_err(|e| e.to_string())?
		.split('\t')
		.next()
		.unwrap_or("");
	let columns: Vec<_> = fields.split_whitespace().collect();
	if columns.len() != 3 || columns[1] != "blob" {
		return Err("diff requires a file path".into());
	}
	let size = store
		.git_str(&["cat-file", "-s", columns[2]])?
		.trim()
		.parse::<u64>()
		.map_err(|e| e.to_string())?;
	if size > max_source() {
		return Err(too_large());
	}
	Ok(Some(Side {
		bytes: store.git(&["cat-file", "blob", columns[2]])?,
		mode: columns[0].into(),
	}))
}

fn disk(root: &Path, path: &str) -> Result<Option<Side>> {
	let path = root.join(path);
	let resolved = match path.canonicalize() {
		Ok(path) => path,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
		Err(e) => return Err(format!("diff disk path: {e}")),
	};
	let root = root.canonicalize().map_err(|e| e.to_string())?;
	if !resolved.starts_with(root) {
		return Err("diff path escapes repository".into());
	}
	let file = std::fs::File::open(&resolved).map_err(|e| e.to_string())?;
	let metadata = file.metadata().map_err(|e| e.to_string())?;
	if !metadata.is_file() {
		return Err("diff requires a regular disk file".into());
	}
	if metadata.len() > max_source() {
		return Err(too_large());
	}
	let mut bytes = Vec::new();
	file.take(max_source() + 1)
		.read_to_end(&mut bytes)
		.map_err(|e| e.to_string())?;
	if bytes.len() as u64 > max_source() {
		return Err(too_large());
	}
	#[cfg(unix)]
	let executable = {
		use std::os::unix::fs::PermissionsExt;
		metadata.permissions().mode() & 0o111 != 0
	};
	#[cfg(not(unix))]
	let executable = false;
	Ok(Some(Side {
		bytes,
		mode: if executable { "100755" } else { "100644" }.into(),
	}))
}

fn metadata(side: &Option<Side>) -> Value {
	match side {
		None => json!({"present":false,"revision":null,"mode":null,"bytes":0}),
		Some(side) => {
			json!({"present":true,"revision":format!("{:x}",Sha256::digest(&side.bytes)),"mode":side.mode,"bytes":side.bytes.len()})
		}
	}
}

fn patch(before: &Option<Side>, after: &Option<Side>, label: &str, path: &str) -> String {
	if metadata(before) == metadata(after) {
		return String::new();
	}
	let empty = Vec::new();
	let a = before.as_ref().map(|s| &s.bytes).unwrap_or(&empty);
	let b = after.as_ref().map(|s| &s.bytes).unwrap_or(&empty);
	let mut out = format!("--- base/{}\n+++ {label}/{}\n", json!(path), json!(path));
	out.push_str(&format!(
		"mode {} -> {}\n",
		before.as_ref().map(|s| s.mode.as_str()).unwrap_or("absent"),
		after.as_ref().map(|s| s.mode.as_str()).unwrap_or("absent")
	));
	let (Ok(a), Ok(b)) = (std::str::from_utf8(a), std::str::from_utf8(b)) else {
		out.push_str("Binary content differs; compare revisions and byte counts.\n");
		return out;
	};
	if a.contains('\0') || b.contains('\0') {
		out.push_str("Binary content differs; compare revisions and byte counts.\n");
		return out;
	}
	let a: Vec<_> = a.split_inclusive('\n').collect();
	let b: Vec<_> = b.split_inclusive('\n').collect();
	let prefix = a.iter().zip(&b).take_while(|(a, b)| a == b).count();
	let suffix = a[prefix..]
		.iter()
		.rev()
		.zip(b[prefix..].iter().rev())
		.take_while(|(a, b)| a == b)
		.count();
	let removed = &a[prefix..a.len() - suffix];
	let added = &b[prefix..b.len() - suffix];
	if removed.is_empty() && added.is_empty() {
		return out;
	}
	out.push_str(&format!(
		"@@ -{},{} +{},{} @@\n",
		prefix + usize::from(!removed.is_empty()),
		removed.len(),
		prefix + usize::from(!added.is_empty()),
		added.len()
	));
	for (sign, lines) in [('-', removed), ('+', added)] {
		for line in lines {
			out.push(sign);
			out.push_str(line);
			if !line.ends_with('\n') {
				out.push_str("\n\\ No newline at end of file\n");
			}
		}
	}
	out
}

pub(super) fn diff(store: &Store, session: &str, input: &Value) -> Result<Value> {
	// No path: the session-wide summary, one row per owned path. `ls` shows
	// the same paths against the branch; this shows them against the disk —
	// whether the overlay agrees with what is checked out, the question a
	// caller asks before materializing.
	let Some(path) = input["path"].as_str() else {
		let owned = store.owned_paths(session)?;
		let tip = store.tip(session)?;
		return success(json!({
			"session": session,
			"branch": store.branch(session),
			"tip": tip,
			"owned": owned.iter().map(|(status, path)| json!({"status": status.to_string(), "path": path})).collect::<Vec<_>>(),
			"consistency": "read_time_comparison",
		}));
	};
	super::resolve_rel(store.root(), path)?;
	let tip = store.tip(session)?;
	let head = store.head()?;
	let base = match (&head, &tip) {
		(Some(head), Some(tip)) => {
			Some(store.git_str(&["merge-base", head, tip])?.trim().to_owned())
		}
		_ => head.clone(),
	};
	let original = object(store, base.as_deref(), path)?;
	let overlay = if tip.is_some() {
		object(store, tip.as_deref(), path)?
	} else {
		original.clone()
	};
	let worktree = disk(store.root(), path)?;
	let original_meta = metadata(&original);
	let overlay_meta = metadata(&overlay);
	let disk_meta = metadata(&worktree);
	let revision = format!(
		"{:x}",
		Sha256::digest(
			json!([path, base, tip, original_meta, overlay_meta, disk_meta])
				.to_string()
				.as_bytes()
		)
	);
	let offset = usize::try_from(input["offset"].as_u64().unwrap_or(0))
		.map_err(|_| "invalid diff offset")?;
	if offset > 0 && input["expected_revision"].as_str().is_none() {
		return Err("diff continuation requires expected_revision".into());
	}
	if input["expected_revision"]
		.as_str()
		.is_some_and(|expected| expected != revision)
	{
		return Err("conflict: diff comparison changed; restart inspection".into());
	}
	// A page under 128 bytes grows to 128 instead of refusing the caller:
	// every page must still move past a multibyte character.
	let limit = (input["limit"].as_u64().unwrap_or(16 * 1024) as usize).max(128);
	let text =
		patch(&original, &overlay, "overlay", path) + &patch(&original, &worktree, "disk", path);
	if offset > text.len() || !text.is_char_boundary(offset) {
		return Err("invalid diff offset".into());
	}
	let mut end = offset.saturating_add(limit).min(text.len());
	while !text.is_char_boundary(end) {
		end -= 1;
	}
	let conflict =
		original_meta != disk_meta && original_meta != overlay_meta && overlay_meta != disk_meta;
	success(
		json!({"path":path,"base_commit":base,"overlay_commit":tip,"base":original_meta,"overlay":overlay_meta,"disk":disk_meta,"conflict":conflict,"disk_changed":original_meta != disk_meta,"overlay_changed":original_meta != overlay_meta,"revision":revision,"patch":&text[offset..end],"offset":offset,"next_offset":if end < text.len() { Some(end) } else { None },"total_patch_bytes":text.len(),"complete":end==text.len(),"consistency":"read_time_comparison"}),
	)
}
