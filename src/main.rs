//! `git-fs mcp` serves the tools over stdio; `git-fs hook <action>` answers a
//! Claude Code lifecycle hook from its JSON payload on stdin; `git-fs call
//! <tool> [json]` runs one tool from the shell. Invoked by git as `pre-push`
//! through the private symlink a push lays, it validates that push instead.

use serde_json::{json, Value};

fn main() {
	if let Some(code) = gitfs::hook::hook_entry() {
		std::process::exit(code);
	}
	let args: Vec<String> = std::env::args().skip(1).collect();
	let code = match args.first().map(String::as_str) {
		Some("mcp") => gitfs::mcp::serve(),
		Some("hook") => gitfs::hooks::run(args.get(1).map(String::as_str).unwrap_or("")),
		Some("call") => call(
			args.get(1).map(String::as_str),
			args.get(2).map(String::as_str),
		),
		Some("tools") => {
			for tool in gitfs::TOOLS {
				println!("{tool}");
			}
			0
		}
		_ => {
			eprintln!("usage: git-fs mcp | hook <session-start|post-edit|read|stop> | call <tool> [json] | tools");
			2
		}
	};
	std::process::exit(code);
}

fn call(tool: Option<&str>, input: Option<&str>) -> i32 {
	let Some(tool) = tool else {
		eprintln!("usage: git-fs call <tool> [json]");
		return 2;
	};
	let input: Value = match input {
		Some(text) => match serde_json::from_str(text) {
			Ok(value) => value,
			Err(e) => {
				eprintln!("input is not JSON: {e}");
				return 2;
			}
		},
		None => json!({}),
	};
	let answer = gitfs::mcp::runtime().block_on(async {
		gitfs::start(gitfs::env_config()?)?;
		let context = gitfs::mcp::context("cli", 1)?;
		gitfs::call(tool, input, &context).await
	});
	match answer {
		Ok(value) => {
			println!(
				"{}",
				serde_json::to_string_pretty(&value).unwrap_or_default()
			);
			i32::from(value["error"] == true)
		}
		Err(e) => {
			eprintln!("{e}");
			1
		}
	}
}
