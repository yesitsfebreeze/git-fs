use super::*;

#[test]
fn patch_targets_include_moves_deletes_spaces_and_normalized_parent_paths() {
	let payload = json!({"tool_input":{"command":"*** Begin Patch\n*** Update File: a.txt\n*** Move to: ../renamed file.txt\n@@\n-*** Add File: fake\n+new\n*** Delete File: ../gone.txt\n*** Add File: /outside/x\n*** Add File: ../.git-fs/private\n*** End Patch"}});
	assert_eq!(
		patch_paths(
			&payload,
			Path::new("/repo"),
			Path::new("/repo/sub"),
			".git-fs"
		),
		["sub/a.txt", "renamed file.txt", "gone.txt"]
	);
}

#[test]
fn routing_replaces_model_context_and_preserves_tool_arguments() {
	let payload = json!({"tool_name":"mcp__git-fs__read", "cwd":"/repo", "tool_input":{"path":"x", "_git_fs":{"session":"wrong","cwd":"/wrong"}}});
	assert!(is_own_tool(&payload));
	let mut input = routed_input(&payload, "right").unwrap();
	let context = take_context(&mut input, 1).unwrap();
	assert_eq!(context.session, "right");
	assert_eq!(context.cwd, "/repo");
	assert_eq!(input, json!({"path":"x"}));
	assert!(!is_own_tool(&json!({"tool_name":"mcp__other__read"})));
}

#[test]
fn codex_requires_valid_routing_instead_of_falling_back_to_main() {
	for mut input in [
		json!({}),
		json!({"_git_fs":{"session":"../bad","cwd":"/repo"}}),
		json!({"_git_fs":{"session":"ok","cwd":"relative"}}),
	] {
		assert!(take_context(&mut input, 1).is_err());
	}
}

#[test]
fn codex_guide_names_patch_and_correct_server() {
	let text = guide("s1", 2, Mode::Git);
	assert!(text.contains("apply_patch"));
	assert!(text.contains("mcp__git_fs__*"));
	assert!(!text.contains("mcp__plugin_"));
	assert!(text.contains("SessionEnd"));
}
