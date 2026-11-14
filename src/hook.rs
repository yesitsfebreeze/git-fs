//! The pre-push hook: `gitfs-hook`, invoked by git through a private `pre-push`
//! symlink, validates the one advertised update and records the verdict. Both
//! the module and the hook executable compile this file.

#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub type Result<T> = std::result::Result<T, String>;

pub(crate) fn git_command_path(path: &Path) -> Result<String> {
	let text = path.to_str().ok_or("hook path invalid")?;
	#[cfg(windows)]
	{
		// Git's hook launcher does not accept Rust's verbatim canonical prefix.
		// UNC paths retain their leading server/share marker.
		let text = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
			format!("//{unc}")
		} else {
			text.strip_prefix(r"\\?\").unwrap_or(text).to_owned()
		};
		Ok(text.replace('\\', "/"))
	}
	#[cfg(not(windows))]
	Ok(text.to_owned())
}

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
	drop(f);
	#[cfg(unix)]
	fs::rename(&temp, path).map_err(|_| "push evidence publication failed")?;
	#[cfg(windows)]
	{
		use std::os::windows::ffi::OsStrExt;
		use windows_sys::Win32::Storage::FileSystem::{
			MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
		};
		let wide = |p: &Path| {
			p.as_os_str()
				.encode_wide()
				.chain(Some(0))
				.collect::<Vec<_>>()
		};
		if unsafe {
			MoveFileExW(
				wide(&temp).as_ptr(),
				wide(path).as_ptr(),
				MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
			)
		} == 0
		{
			return Err("push evidence publication failed".into());
		}
	}
	sync_directory(path.parent().ok_or("missing push evidence directory")?)
}

#[cfg(unix)]
pub(crate) fn sync_directory(path: &Path) -> Result<()> {
	File::open(path)
		.and_then(|f| f.sync_all())
		.map_err(|_| "push evidence directory sync failed".into())
}

#[cfg(windows)]
pub(crate) fn sync_directory(_: &Path) -> Result<()> {
	// Windows publishes through MoveFileExW(WRITE_THROUGH) above. Directories
	// cannot be flushed through Rust's File::sync_all as they can on Unix.
	Ok(())
}

fn user_hook(bytes: &[u8], root: &Path, result: &Path) -> Result<bool> {
	let hooks = std::env::var_os("GITFS_PUSH_USER_HOOKS").ok_or("original hooks missing")?;
	let remote = std::env::var_os("GITFS_PUSH_REMOTE").ok_or("push remote missing")?;
	let url = std::env::args_os()
		.nth(2)
		.ok_or("push destination missing")?;
	let input = result.with_file_name("hook-input");
	fs::write(&input, bytes).map_err(|_| "hook input unavailable")?;
	let mut command = Command::new("git");
	// Drop the private invocation's hooksPath and event suppression, retaining
	// any configuration parameters the caller supplied before that invocation.
	command.env_remove("GIT_CONFIG_PARAMETERS");
	if let Some(parameters) = std::env::var_os("GITFS_PUSH_ORIGINAL_CONFIG_PARAMETERS") {
		command.env("GIT_CONFIG_PARAMETERS", parameters);
	}
	let status = command
		.arg("-C")
		.arg(root)
		.arg("-c")
		.arg(format!(
			"core.hooksPath={}",
			git_command_path(Path::new(&hooks))?
		))
		.args(["hook", "run", "--ignore-missing"])
		.arg(format!("--to-stdin={}", git_command_path(&input)?))
		.args(["pre-push", "--"])
		.arg(remote)
		.arg(url)
		.env_remove("GITFS_PUSH_HOOK")
		.stdin(Stdio::null())
		.status()
		.map_err(|_| "original pre-push hook unavailable")?;
	Ok(status.success())
}

/// Invoked only through our private pre-push executable symlink; the
/// module carries it for the hook binary.
#[allow(dead_code)]
pub fn hook_entry() -> Option<i32> {
	if std::env::var("GITFS_PUSH_HOOK").as_deref() != Ok("1")
		|| std::env::args_os()
			.next()
			.and_then(|s| PathBuf::from(s).file_stem().map(|s| s == "pre-push"))
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
		let path = std::env::var_os("GITFS_PUSH_RESULT").ok_or("hook result missing")?;
		let mut valid = hook_valid(&bytes, &old, &new, &reference);
		if valid {
			let root = std::env::var_os("GITFS_REVIEW_ROOT")
				.map(PathBuf::from)
				.or_else(|| std::env::current_dir().ok())
				.ok_or("review repository unavailable")?;
			if let Err(error) = crate::review::check_range(&root, &old, &new) {
				eprintln!("{error}");
				valid = false;
			}
			if valid {
				valid = user_hook(&bytes, &root, Path::new(&path)).unwrap_or(false);
			}
		}
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
