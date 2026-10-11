//! The pre-push hook: `gitfs-hook`, invoked by git through a private `pre-push`
//! symlink, validates the one advertised update and records the verdict. Both
//! the module and the hook executable compile this file.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, String>;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
	let temp = path.with_extension("tmp");
	let mut opts = OpenOptions::new();
	opts.create(true).truncate(true).write(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		opts.mode(0o600);
	}
	let mut f = opts
		.open(&temp)
		.map_err(|_| "push evidence cannot be written")?;
	f.write_all(bytes)
		.and_then(|_| f.sync_all())
		.map_err(|_| "push evidence durability failed")?;
	fs::rename(&temp, path).map_err(|_| "push evidence publication failed")?;
	File::open(path.parent().ok_or("missing push evidence directory")?)
		.and_then(|f| f.sync_all())
		.map_err(|_| "push evidence directory sync failed".into())
}

/// Invoked only through our private pre-push executable symlink; the
/// module carries it for the hook binary.
#[allow(dead_code)]
pub fn hook_entry() -> Option<i32> {
	if std::env::var("GITFS_PUSH_HOOK").as_deref() != Ok("1")
		|| std::env::args_os()
			.next()
			.and_then(|s| PathBuf::from(s).file_name().map(|s| s == "pre-push"))
			!= Some(true)
	{
		return None;
	}
	let result = (|| -> Result<bool> {
		let mut bytes = Vec::new();
		std::io::stdin()
			.take(8193)
			.read_to_end(&mut bytes)
			.map_err(|_| "hook read failed")?;
		let old = std::env::var("GITFS_PUSH_OLD").map_err(|_| "hook old missing")?;
		let new = std::env::var("GITFS_PUSH_NEW").map_err(|_| "hook new missing")?;
		let reference = std::env::var("GITFS_PUSH_REF").map_err(|_| "hook ref missing")?;
		let valid = hook_valid(&bytes, &old, &new, &reference);
		let path = std::env::var_os("GITFS_PUSH_RESULT").ok_or("hook result missing")?;
		atomic_write(
			Path::new(&path),
			if valid { b"validated" } else { b"refused" },
		)?;
		Ok(valid)
	})();
	Some(if result == Ok(true) { 0 } else { 1 })
}

pub fn hook_valid(bytes: &[u8], old: &str, new: &str, reference: &str) -> bool {
	if bytes.len() > 8192 {
		return false;
	}
	let Ok(text) = std::str::from_utf8(bytes) else {
		return false;
	};
	let lines: Vec<_> = text.lines().collect();
	if lines.len() != 1 {
		return false;
	}
	let fields: Vec<_> = lines[0].split(' ').collect();
	fields.len() == 4 && fields[1] == new && fields[2] == reference && fields[3] == old
}
