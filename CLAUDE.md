# git-fs project rules

## File editing

git-fs is the source of truth. Disk is read-only during a session.

**Never use Edit or Write tools.** They are denied in settings.json.

Instead:
- Read   -> `git_fs_read ref:<agent-branch> path:<file>` (add start_line/end_line for slices)
- Edit   -> `git_fs_replace branch:<agent-branch> path:<file> old_str:"..." new_str:"..."` (preferred — text-match, immune to line-number drift)
- Edit   -> `git_fs_patch branch:<agent-branch> path:<file> start_line:N end_line:M content:"..."` (line-range fallback)
- Create -> `git_fs_write branch:<agent-branch> path:<file> content:"..."`

Your agent branch is injected at session start as `Branch: agent/<session-id>`.

Disk is only updated at session end — the Stop hook merges your branch into main and runs `checkout main -> cwd`.

## Multi-agent coordination

Multiple agent branches may touch the same files concurrently. Before editing a shared file:

1. `git_fs_branch_list` — see other active `agent/*` branches.
2. `git_fs_log branch:<other-agent>` + `git_fs_diff` on files you plan to touch — see what they changed and why.
3. **Always align to the latest version.** Base your edits on the most recent commit across all agent branches for that file, not just your own branch's view. This avoids merge races where two agents fork from stale state and clobber each other at Stop-hook merge time.
4. If another agent already implemented something similar, mirror their pattern rather than inventing a parallel approach.

## Exception

`/merge` skill: uses `git_fs_merge` + `git_fs_checkout` directly. No Edit/Write needed there either.
