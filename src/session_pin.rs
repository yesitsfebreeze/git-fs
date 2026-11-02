//! Which session a process serves when several agents share one checkout.
//!
//! `session-start` used to pin only `<store>/CURRENT`; two concurrent hosts in
//! one repository overwrote it and the later one won both MCP servers. The
//! hook now also writes the session against each ancestor process id, and the
//! MCP server reads the first id in its own ancestry that has a pin. Shell
//! wrappers around either process are why the chain, not the parent, is used.

use std::path::{Path, PathBuf};

/// Deep enough for a terminal, a shell wrapper and the host process.
const MAX_ANCESTORS: usize = 8;
/// A pin older than this is a reused process id, not a live session.
const PIN_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// This process and its ancestors, nearest first.
pub fn ancestors() -> Vec<u32> {
	let mut ids = vec![std::process::id()];
	#[cfg(unix)]
	{
		// getppid answers only this process; `ps` walks the chain on every
		// Unix that has it. Where ps is absent, the parent alone still covers
		// a directly spawned MCP server.
		let mut pid = unsafe { libc::getppid() } as u32;
		while pid > 1 && ids.len() < MAX_ANCESTORS && !ids.contains(&pid) {
			ids.push(pid);
			pid = match unix_parent(pid) {
				Some(parent) => parent,
				None => break,
			};
		}
	}
	#[cfg(windows)]
	{
		let parents = parent_map();
		let mut pid = std::process::id();
		while let Some(&parent) = parents.get(&pid) {
			if parent == 0 || ids.len() >= MAX_ANCESTORS || ids.contains(&parent) {
				break;
			}
			ids.push(parent);
			pid = parent;
		}
	}
	ids
}

/// Write `session` for every ancestor: the host's own id pins it, and a
/// wrapper between host and MCP server is covered too.
pub fn pin(dir: &Path, session: &str) -> crate::Result<()> {
	let dir = dir.join("sessions");
	std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create session pins: {e}"))?;
	for id in ancestors() {
		write_pin(&dir.join(format!("{id}.session")), session)?;
	}
	Ok(())
}

/// The session pinned for this process's ancestry, if any.
pub fn resolve(dir: &Path) -> Option<String> {
	resolve_from(&dir.join("sessions"), &ancestors())
}

fn resolve_from(dir: &Path, ids: &[u32]) -> Option<String> {
	for id in ids {
		let path = dir.join(format!("{id}.session"));
		let Ok(metadata) = std::fs::metadata(&path) else {
			continue;
		};
		let fresh = metadata
			.modified()
			.ok()
			.and_then(|at| at.elapsed().ok())
			.is_none_or(|age| age < PIN_MAX_AGE);
		if !fresh {
			continue;
		}
		let Ok(text) = std::fs::read_to_string(&path) else {
			continue;
		};
		let session = text.lines().next().unwrap_or_default().trim();
		if crate::provenance::valid_session(session) {
			return Some(session.to_owned());
		}
	}
	None
}

fn write_pin(path: &Path, session: &str) -> crate::Result<()> {
	let temp = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
	std::fs::write(&temp, format!("{session}\n"))
		.map_err(|e| format!("cannot write session pin: {e}"))?;
	std::fs::rename(&temp, path).map_err(|e| {
		let _ = std::fs::remove_file(&temp);
		format!("cannot publish session pin: {e}")
	})
}

#[cfg(unix)]
fn unix_parent(pid: u32) -> Option<u32> {
	let output = std::process::Command::new("ps")
		.args(["-o", "ppid=", "-p", &pid.to_string()])
		.output()
		.ok()?;
	if !output.status.success() {
		return None;
	}
	let parent = String::from_utf8_lossy(&output.stdout)
		.trim()
		.parse::<u32>()
		.ok()?;
	(parent > 1 && parent != pid).then_some(parent)
}

#[cfg(windows)]
fn parent_map() -> std::collections::HashMap<u32, u32> {
	use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
	use windows_sys::Win32::System::Diagnostics::ToolHelp::{
		CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
		TH32CS_SNAPPROCESS,
	};
	let mut map = std::collections::HashMap::new();
	let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
	if snapshot == INVALID_HANDLE_VALUE {
		return map;
	}
	let mut entry = PROCESSENTRY32W {
		dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
		..PROCESSENTRY32W::default()
	};
	let mut more = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
	while more {
		map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
		more = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
	}
	unsafe { CloseHandle(snapshot) };
	map
}

#[cfg(test)]
#[path = "../tests/unit/session_pin.rs"]
mod tests;
