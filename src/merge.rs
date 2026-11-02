//! Three-way text merge for one source edited by several sessions.
//!
//! `catchup` and `materialize` can meet two committed revisions of one path.
//! Git's own merge-file is the merge engine: it is already required at
//! runtime, understands the same line endings and diff algorithm as the
//! repository, and reports overlapping edits without inventing an answer.

use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Merge `ours` and `theirs` on top of their common `base`.
///
/// `Ok(Some(bytes))` is a clean merge: neither side changed the same region.
/// `Ok(None)` means the changes overlap, or a side is binary or too large to
/// merge — the caller keeps its existing refusal instead of guessing.
pub fn merge3(base: &[u8], ours: &[u8], theirs: &[u8]) -> crate::Result<Option<Vec<u8>>> {
	if ours == theirs {
		return Ok(Some(ours.to_vec()));
	}
	if base == ours {
		return Ok(Some(theirs.to_vec()));
	}
	if base == theirs {
		return Ok(Some(ours.to_vec()));
	}
	let cap = crate::limits::limits().source_bytes as usize;
	if [base, ours, theirs]
		.iter()
		.any(|side| side.len() > cap || side.contains(&0))
	{
		return Ok(None);
	}
	// Two sessions appending rows to one table add both at the same anchor,
	// which a line diff reports as an overlapping edit. When each side is the
	// base plus appended lines, the append is the change: keep both, ours
	// first. A side that rewrote or removed anything is not a superset and
	// stays with the line merge below.
	if ours.starts_with(base)
		&& theirs.starts_with(base)
		&& (base.is_empty() || base.ends_with(b"\n"))
	{
		let mut merged = base.to_vec();
		merged.extend_from_slice(&ours[base.len()..]);
		merged.extend_from_slice(&theirs[base.len()..]);
		return Ok(Some(merged));
	}

	let dir = std::env::temp_dir().join(format!(
		"gitfs-merge-{}-{}",
		std::process::id(),
		NEXT.fetch_add(1, Ordering::Relaxed)
	));
	let result = merge_in(&dir, base, ours, theirs);
	let _ = std::fs::remove_dir_all(&dir);
	result
}

fn merge_in(
	dir: &std::path::Path,
	base: &[u8],
	ours: &[u8],
	theirs: &[u8],
) -> crate::Result<Option<Vec<u8>>> {
	std::fs::create_dir_all(dir).map_err(|e| format!("merge workspace: {e}"))?;
	let write = |name: &str, bytes: &[u8]| -> crate::Result<std::path::PathBuf> {
		let path = dir.join(name);
		let mut file =
			std::fs::File::create(&path).map_err(|e| format!("merge input {name}: {e}"))?;
		file.write_all(bytes)
			.map_err(|e| format!("merge input {name}: {e}"))?;
		Ok(path)
	};
	let ours_path = write("ours", ours)?;
	let base_path = write("base", base)?;
	let theirs_path = write("theirs", theirs)?;
	let output = std::process::Command::new("git")
		.args(["merge-file", "-p", "--diff3"])
		.args(["-L", "ours", "-L", "base", "-L", "theirs"])
		.arg(&ours_path)
		.arg(&base_path)
		.arg(&theirs_path)
		.output()
		.map_err(|e| format!("git merge-file failed to start: {e}"))?;
	if output.status.success() {
		return Ok(Some(output.stdout));
	}
	match output.status.code() {
		// Git answers the number of conflicts (1..=127).
		Some(code) if (1..=127).contains(&code) => Ok(None),
		_ => Err(format!(
			"git merge-file failed: {}",
			String::from_utf8_lossy(&output.stderr).trim()
		)),
	}
}

#[cfg(test)]
#[path = "../tests/unit/merge.rs"]
mod tests;
