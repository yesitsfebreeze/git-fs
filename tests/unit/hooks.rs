//! The pre-tool verdicts and the session-start guide: pure text, no store.

use super::*;

const S: &str = "s1";

fn v(tool: &str, input: Value, rel: Option<&str>) -> Verdict {
	verdict(tool, &input, rel, S)
}

#[test]
fn guide_names_every_tool_family_and_the_prefix() {
	let text = guide(S, 3, Mode::Git);
	for word in [
		"refs/gitfs/s1",
		"3 owned path(s)",
		"mcp__plugin_git-fs_git-fs__",
		"- read,",
		"- edit and write,",
		"- grep, glob and search,",
		"gitfs_materialize",
		"gitfs_catchup",
		"integrate",
		"push",
	] {
		assert!(text.contains(word), "guide lacks {word:?}:\n{text}");
	}
	assert!(!text.contains("Mode local"));
	assert!(guide(S, 0, Mode::Local).contains("Mode local"));
}

#[test]
fn native_write_and_edit_inside_the_repository_are_denied() {
	for tool in ["Write", "Edit", "MultiEdit"] {
		match v(tool, json!({"file_path": "src/a.rs"}), Some("src/a.rs")) {
			Verdict::Deny(reason) => {
				assert!(reason.contains("src/a.rs"), "{reason}");
				assert!(reason.contains("refs/gitfs/s1"), "{reason}");
				assert!(reason.contains(tool), "{reason}");
			}
			other => panic!("{tool}: {other:?}"),
		}
	}
}

#[test]
fn outside_the_repository_everything_is_allowed() {
	for tool in ["Write", "Edit", "Read", "Grep", "Glob"] {
		assert_eq!(
			v(tool, json!({"file_path": "/etc/hosts"}), None),
			Verdict::Allow
		);
	}
}

#[test]
fn text_reads_get_a_note_and_binary_reads_do_not() {
	assert!(matches!(
		v("Read", json!({"file_path": "src/a.rs"}), Some("src/a.rs")),
		Verdict::Note(text) if text.contains("git-fs read src/a.rs")
	));
	for binary in ["img.PNG", "doc.pdf", "nb.ipynb", "x.jpeg"] {
		assert_eq!(
			v("Read", json!({"file_path": binary}), Some(binary)),
			Verdict::Allow
		);
	}
	assert_eq!(
		v(
			"Read",
			json!({"file_path": "a.txt", "pages": "1-3"}),
			Some("a.txt")
		),
		Verdict::Allow
	);
}

#[test]
fn grep_and_glob_inside_get_a_note_naming_the_git_fs_tool() {
	assert!(matches!(
		v("Grep", json!({"pattern": "x"}), Some(".")),
		Verdict::Note(text) if text.contains("git-fs grep") && text.contains("native Grep")
	));
	assert!(matches!(
		v("Glob", json!({"pattern": "**/*.rs"}), Some(".")),
		Verdict::Note(text) if text.contains("git-fs glob")
	));
}

#[test]
fn bash_file_readers_are_redirected_and_everything_else_passes() {
	assert_eq!(bash_redirect("cat src/a.rs"), Some(("cat", "read")));
	assert_eq!(
		bash_redirect("sed -n 1,40p src/a.rs"),
		Some(("sed", "read"))
	);
	assert_eq!(bash_redirect("  rg -n foo src"), Some(("rg", "grep")));
	assert_eq!(bash_redirect("find . -name '*.rs'"), Some(("find", "glob")));
	assert_eq!(
		bash_redirect("cd ~/dev/x && rg pattern"),
		Some(("rg", "grep"))
	);
	assert_eq!(
		bash_redirect("cd a; cd b && head -5 f"),
		Some(("head", "read"))
	);
	assert_eq!(
		bash_redirect("cat > f.txt <<'EOF'\nhi\nEOF"),
		Some(("cat", "write"))
	);
	assert_eq!(
		bash_redirect("tee -a f.txt <<EOF\nhi\nEOF"),
		Some(("tee", "write"))
	);
	for pass in [
		"cargo test --locked",
		"git status --short",
		"just build",
		"cd x && cargo build",
		"cd x",
		"",
		"echo hi | tee log",
	] {
		assert_eq!(bash_redirect(pass), None, "{pass:?}");
	}
}

#[test]
fn bash_verdicts_carry_the_word_and_the_replacement() {
	assert!(matches!(
		v("Bash", json!({"command": "cat src/a.rs"}), None),
		Verdict::Note(text) if text.contains("`cat`") && text.contains("git-fs read")
	));
	assert!(matches!(
		v("Bash", json!({"command": "cat > f <<EOF\nx\nEOF"}), None),
		Verdict::Note(text) if text.contains("git-fs write")
	));
	assert_eq!(
		v("Bash", json!({"command": "cargo test"}), None),
		Verdict::Allow
	);
	assert_eq!(v("Bash", json!({}), None), Verdict::Allow);
	assert_eq!(
		v(
			"NotebookEdit",
			json!({"file_path": "n.ipynb"}),
			Some("n.ipynb")
		),
		Verdict::Allow
	);
}
