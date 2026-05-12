---
name: git-fs
description: Virtual filesystem over a bare git object store. Each Claude Code session works on its own agent/<session-id> branch; every edit is a commit; Stop hook merges to main and materializes to disk. Use when the project has a .git-fs/ directory and the git-fs MCP is registered. Triggers on file edits in such a project, on questions about session branches, sibling agents, mergeignore, or git-fs tool selection.
---

# git-fs

Virtual filesystem over a bare git repository. Disk is read-only during a session. All file changes go through git-fs MCP tools and become git commits on a session-scoped branch. The Stop hook merges that branch into `main` and materializes the result to disk.

## When this skill applies

This skill is for projects that use git-fs as their file backend. Signs:

- `.git-fs/` directory exists at project root.
- `.mcp.json` contains a `git-fs` server entry.
- Session-start banner mentions `Branch: agent/<session-id>`.

If those signs are absent and the user wants this behavior, follow the install flow at
https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/install.md.

## Core rule

**Never use the native Edit or Write tools.** They are denied in `.claude/settings.json` on git-fs projects by design. All edits flow through the MCP tools below. Read is allowed because the PreToolUse hook materializes git-fs content to disk first.

## Tool selection

| Need | Tool | Notes |
|------|------|-------|
| Read full file | `git_fs_read ref:<branch> path:<file>` | |
| Read line slice | `git_fs_read ref:<branch> path:<file> start_line:N end_line:M` | 1-indexed inclusive |
| Edit existing (preferred) | `git_fs_replace branch:<branch> path:<file> old_str:"..." new_str:"..."` | Text-match, drift-immune; include enough context to make `old_str` unique |
| Edit by line range | `git_fs_patch branch:<branch> path:<file> start_line:N end_line:M content:"..."` | Use when `old_str` cannot be made unique |
| Create file | `git_fs_write branch:<branch> path:<file> content:"..."` | Overwrites if exists |
| Delete file | `git_fs_rm branch:<branch> path:<file>` | |
| List files | `git_fs_ls ref:<ref> path:<prefix> recursive:true` | |
| Diff two refs | `git_fs_diff ref_a:<a> ref_b:<b>` | |
| Branch history | `git_fs_log branch:<branch>` | |
| Materialize to disk | `git_fs_checkout ref:<ref> dest:<dir>` | Usually automatic on Stop |
| 3-way merge | `git_fs_merge ours:<a> theirs:<b> base:<c> into:<branch>` | Usually automatic on Stop |
| List sibling sessions | `git_fs_branch_list` | |
| Create branch | `git_fs_branch_create name:<name> from:<ref>` | |
| Delete branch | `git_fs_branch_delete name:<name>` | |

The agent branch for the current session is injected at session start in the banner as `Branch: agent/<uuid>`. Use it as the `branch` argument for write/edit/rm.

## Decision flow for edits

1. Reading an unknown file? `git_fs_read` first.
2. Changing existing content? `git_fs_replace` (text-match). Falls back to `git_fs_patch` only when the target text isn't unique.
3. Creating new content? `git_fs_write`.
4. Don't re-read after a successful edit — git-fs commits atomically; the change is visible to subsequent tools immediately.

## Multi-agent coordination

Multiple Claude Code sessions can run against the same git-fs repo in parallel.

- Each session works on its own `agent/<session-id>` branch — full isolation.
- Discover other sessions: `git_fs_branch_list`.
- Read another session's intent: `git_fs_read ref:agent/<other-id> path:.git-fs/session/intent.md`.
- Mergeignore paths are stripped before a session's branch merges to `main`. Hard defaults: `.agent`, `CONFLICTS.md`. Project-configurable extras: `.git-fs/mergeignore`.

Spec: https://github.com/yesitsfebreeze/git-fs/blob/main/docs/multi-agent-session.md.

## Stop hook

The Stop hook does, in order:

1. Strip mergeignored paths from the agent branch.
2. Acquire `<.git-fs>/merge.lock` (serializes concurrent Stop hooks).
3. 3-way merge `agent/<id>` into `main` using the base OID stored at session start.
4. Checkout `main` to disk.
5. On conflict: write `CONFLICTS.md` on the agent branch; do not merge.

If you see `CONFLICTS.md` referenced in a sibling, that session ended in a merge conflict — pick it up or rebase.

## Worktrees

git-fs works across git worktrees in two modes.

- **Per-worktree (default).** Each worktree has its own `.git-fs/`. Sessions are isolated per worktree — `git_fs_branch_list` only shows agents in this worktree.
- **Shared store.** Every worktree's `.mcp.json` sets `env.GIT_FS_REPO` to one absolute path. All worktrees see the same branch graph; `git_fs_branch_list` returns every active agent across every worktree. The merge lock is global. Stop still materializes `main` into each session's own `cwd`, so disk contents stay per-worktree even though history is shared.

Detect shared mode: read the `git-fs` entry of the project's `.mcp.json`. If `env.GIT_FS_REPO` is an absolute path outside the current worktree, you're in shared mode — siblings may be in other directories.

## Installation

If `git-fs` MCP isn't registered yet on the user's machine, follow:
https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/install.md

## Updating

https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/update.md
