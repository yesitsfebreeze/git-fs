//! A Windows job owns Git and every helper before any of them can run.
use crate::Result;
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::process::{Child, Command};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
	CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
	AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
	SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
	JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{
	OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

struct Handle(HANDLE);
impl Drop for Handle {
	fn drop(&mut self) {
		unsafe { CloseHandle(self.0) };
	}
}

pub(super) struct Job(Handle);
impl Job {
	pub(super) fn prepare(command: &mut Command) -> Result<Self> {
		let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
		if handle.is_null() {
			return Err("Git process job unavailable".into());
		}
		let job = Self(Handle(handle));
		let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
		limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
		if unsafe {
			SetInformationJobObject(
				handle,
				JobObjectExtendedLimitInformation,
				std::ptr::from_ref(&limits).cast(),
				std::mem::size_of_val(&limits) as u32,
			)
		} == 0
		{
			return Err("Git process job limits unavailable".into());
		}
		// Assigning a running child has a race: it can create an unowned helper.
		command.creation_flags(CREATE_SUSPENDED);
		Ok(job)
	}

	pub(super) fn attach_and_resume(&self, child: &mut Child) -> Result<()> {
		let result = (|| {
			if unsafe { AssignProcessToJobObject(self.0 .0, child.as_raw_handle()) } == 0 {
				return Err("Git process job assignment failed".into());
			}
			let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
			if snapshot == INVALID_HANDLE_VALUE {
				return Err("Git suspended thread unavailable".into());
			}
			let snapshot = Handle(snapshot);
			let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
			entry.dwSize = std::mem::size_of_val(&entry) as u32;
			let mut next = unsafe { Thread32First(snapshot.0, &mut entry) };
			while next != 0 {
				if entry.th32OwnerProcessID == child.id() {
					let thread =
						unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
					if thread.is_null() {
						return Err("Git suspended thread unavailable".into());
					}
					let thread = Handle(thread);
					return if unsafe { ResumeThread(thread.0) } == u32::MAX {
						Err("Git process resume failed".into())
					} else {
						Ok(())
					};
				}
				next = unsafe { Thread32Next(snapshot.0, &mut entry) };
			}
			Err("Git suspended thread unavailable".into())
		})();
		if result.is_err() {
			let _ = child.kill();
			let _ = child.wait();
		}
		result
	}

	pub(super) fn terminate(&self) {
		unsafe { TerminateJobObject(self.0 .0, 1) };
	}
}
