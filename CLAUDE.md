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

## Exception

`/merge` skill: uses `git_fs_merge` + `git_fs_checkout` directly. No Edit/Write needed there either.
