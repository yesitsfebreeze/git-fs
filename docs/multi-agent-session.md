# Multi-Agent Session Coordination

Lightweight design for many agents working in parallel on the same git-fs repo
without locks, leases, or central coordination.

## Goals

- Many agents work simultaneously on their own branches.
- Agents can discover and read each other's in-flight work.
- Merges to main are clean, semantic conflicts caught pre-merge.
- Main stays free of per-session scratch.
- Full audit trail via git history.

## Non-goals

- Hard locking. Coordination is advisory.
- Hunk-level concurrency on the same file.
- Real-time collaboration. Granularity = branch.

## Mechanism

### 1. Per-session scratch folder

Path: `.git-fs/session/`

Owned by the current agent. Contains anything the agent wants siblings or
future sessions to see:

- `intent.md` — what this session is doing, why, which paths it touches.
- `notes.md` — running notes, decisions, blockers.
- `paths.md` — explicit list of files/globs being modified.
- anything else the agent finds useful.

Lifecycle:

- **SessionStart hook** clears `.git-fs/session/` and seeds an empty
  `intent.md` template.
- Agent writes to it freely during the session (commits land on agent branch).
- Sibling agents read via `git_fs_read ref:agent/<other-id> path:.git-fs/session/intent.md`.
- **Stop hook** strips the folder before merging into main (see mergeignore).
- Folder dies with the agent branch.

### 2. Mergeignore

Path: `.git-fs/mergeignore`

Same syntax as `.gitignore`. Listed paths are present on agent branches but
never merged to main.

Default contents:

```
.git-fs/session/
.git-fs/scratch/
*.tmp
```

Implementation in Stop hook merge step:

```
1. Read .git-fs/mergeignore from main.
2. On agent branch, before merge:
     - rm matched paths
     - commit "strip mergeignored paths"
3. Merge agent branch into main.
```

Alt impl: `git merge -s ours` for ignored paths, normal merge for the rest.
Pick whichever is simpler in the Rust merge code.

### 3. Sibling discovery

Already supported:

- `git_fs_branch_list` — enumerate live agent branches.
- `git_fs_read ref:agent/<id> path:.git-fs/session/intent.md` — read sibling intent.
- `git_fs_diff` — compare branches.

No new tool needed.

### 4. Pre-merge reconcile (Stop hook addition)

Before merging agent branch into main:

1. List active sibling branches (`git_fs_branch_list`).
2. For each sibling:
   - Read its `.git-fs/session/intent.md` and `paths.md`.
   - If overlapping paths: agent reads sibling diff on those paths,
     decides whether to wait, rebase, or proceed.
3. Merge.

Initial impl can skip the reasoning step and just log overlaps to the user.
Reasoning agent comes later.

### 5. Merge serialization

Two Stop hooks firing concurrently race on main. Need a merge lock:

- File lock on `.git-fs/merge.lock` (flock or equivalent).
- Or CAS retry loop on `refs/heads/main`.

File lock is simpler; pick that.

## Audit

Every commit already carries the agent branch name. To make session queries
trivial, add a git trailer to commits made via git-fs tools:

```
Session-Id: agent/<uuid>
```

Then `git log --grep="Session-Id: agent/X"` returns the full session.

This is a nice-to-have, not required for the core mechanism.

## Compaction

Git packfiles already compact. Run `git gc` periodically (cron or on-demand
via a tool). Old merged session branches get deleted after N days by a
sweeper:

```
git_fs_branch_list → for each agent/* branch merged > 7 days ago → delete
```

## What this replaces

- No empty-commit leases.
- No `refs/leases/<path>` locks.
- No TTL/heartbeat reaper.
- No per-file concurrency primitives.

Branches are the unit of isolation. Mergeignore keeps main clean. Session
folder is the broadcast channel.

## Implementation tasks

1. **Spec** — this doc. (done)
2. **Default `.git-fs/mergeignore`** — check into repo with sensible defaults.
3. **SessionStart hook** — clear `.git-fs/session/`, seed `intent.md` template.
4. **Stop hook merge** — respect `.git-fs/mergeignore` (strip before merge).
5. **Stop hook reconcile** — log sibling-branch path overlaps before merge.
6. **Merge lock** — file lock on `.git-fs/merge.lock`.
7. **(Optional) Session-Id trailer** — inject in git-fs commit path.
8. **(Optional) Branch sweeper tool** — prune old merged agent branches.

Order: 2 → 3 → 4 → 6 → 5 → 7 → 8.

## Open questions

- Where does SessionStart hook live for git-fs (Claude Code hook vs git-fs
  internal init)? Probably Claude Code SessionStart calling a git-fs init
  command.
- Mergeignore parser — reuse a gitignore crate or roll minimal glob match?
  Reuse if cheap.
- What happens if two agents both modify `.git-fs/mergeignore` itself?
  Treat as normal file conflict; resolve manually.
