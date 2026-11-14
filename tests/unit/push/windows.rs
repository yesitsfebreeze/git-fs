use super::*;

#[test]
fn windows_timeout_and_output_limit_remain_bounded() {
	let temp = tempfile::tempdir().unwrap();
	let cancel = AtomicBool::new(false);
	let cx = Context {
		root: temp.path().into(),
		dir: temp.path().join("state"),
		deadline: Instant::now() + Duration::from_millis(300),
		cancel: &cancel,
	};
	let mut command = Command::new("cmd.exe");
	command.args(["/d", "/c", "echo SECRET & ping -n 30 127.0.0.1"]);
	let start = Instant::now();
	let error = cx.run(command).err().unwrap();
	assert!(error.contains("deadline"), "{error}");
	assert!(!error.contains("SECRET"));
	assert!(start.elapsed() < Duration::from_secs(3));
	let cx = Context {
		deadline: Instant::now() + Duration::from_secs(10),
		..cx
	};
	let mut command = Command::new("powershell.exe");
	command.args([
		"-NoProfile",
		"-NonInteractive",
		"-Command",
		&format!(
			"[Console]::Out.Write(('SECRET' * {}))",
			output_limit() / 6 + 4096
		),
	]);
	let error = cx.run(command).err().unwrap();
	assert!(error.contains("limit"), "{error}");
	assert!(!error.contains("SECRET"));
}

#[test]
fn windows_cancellation_kills_git_helpers() {
	use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
	use windows_sys::Win32::System::Threading::{
		OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
	};
	let temp = tempfile::tempdir().unwrap();
	let pidfile = temp.path().join("helper.pid");
	let path = pidfile.to_string_lossy().replace('\'', "''");
	let cancel = Arc::new(AtomicBool::new(false));
	let flag = cancel.clone();
	let root = temp.path().to_path_buf();
	let worker = std::thread::spawn(move || {
		let cx = Context {
			root: root.clone(),
			dir: root,
			deadline: Instant::now() + Duration::from_secs(10),
			cancel: &flag,
		};
		let mut command = Command::new("powershell.exe");
		command.args(["-NoProfile", "-NonInteractive", "-Command", &format!(
			"$p = Start-Process -FilePath ping.exe -ArgumentList @('-n','30','127.0.0.1') -PassThru -NoNewWindow; [System.IO.File]::WriteAllText('{path}', [string]$p.Id); Wait-Process -Id $p.Id"
		)]);
		cx.run(command).err().unwrap()
	});
	let until = Instant::now() + Duration::from_secs(5);
	while !pidfile.exists() && Instant::now() < until {
		std::thread::sleep(Duration::from_millis(10));
	}
	let pid = fs::read_to_string(&pidfile).unwrap().parse().unwrap();
	let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
	assert!(!process.is_null(), "helper did not start");
	cancel.store(true, Ordering::SeqCst);
	assert!(worker.join().unwrap().contains("cancelled"));
	let outcome = unsafe { WaitForSingleObject(process, 2000) };
	unsafe { CloseHandle(process) };
	assert_eq!(outcome, WAIT_OBJECT_0, "helper survived cancellation");
}
