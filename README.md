# git-fs

**Git session branches for AI agents.** One Rust binary, one MCP server, four
Claude Code hooks. Every write an agent makes is a commit on its own branch
`refs/gitfs/<session>`; the worktree changes only when a materialize says so.

v4 is the kern `fs` extension pulled out of [kern](https://github.com/yesitsfebreeze/kern)
and unified with the v3 hooks. It replaces the Node implementation.

## What you get

- **Guarded writes.** `write` and `edit` are refused unless the bytes the
  session last read are still current. A whole-file read of more than 350 lines
  is refused; read a range or grep. `edit` replaces one exact unique match, all
  matches with `all`, or an `edits` array in one all-or-nothing commit.
- **Branch-first reads and search.** `read`, `glob`, `grep` and `search`
  (typed ripgrep stages with cursors) see the session's changes before the
  worktree, so an agent searches what it wrote without materializing.
- **The session branch.** `gitfs_ls`, `gitfs_diff`, `gitfs_rollback` (one
  commit, or one path back to session start), `gitfs_catchup` (rebase onto
  moved trunk after checking which owned paths trunk changed under you),
  `gitfs_materialize` (copy owned paths to disk, refusing files newer on disk or
  modified since the last materialize, `force` to override), `gitfs_capture`
  (pull disk edits of owned paths back in).
- **Publication.** `integrate` previews and commits the session onto trunk
  with a secret scan, `push` runs a guarded push with `operation_id`
  idempotency and a private pre-push hook, `undo` reverts the last integrated
  change, `scan` checks files for secrets.
- **Native tools covered by hooks.** A Claude `Write` or `Edit` is mirrored
  onto the branch (`post-edit`); a `Read` of a path the branch changed gets that
  path onto disk first (`read`); `Stop` materializes every owned path and
  reports what it left alone. `SessionStart` pins the session the MCP server
  serves.

Nothing touches your project's `.git` except `refs/gitfs/*`, and `.git/info/exclude`
gets one line for the store directory `.git-fs/` (session indexes, read
records and the materialize ledger).

## Install

Needs `cargo` to build and `git` and `rg` (ripgrep) at runtime.

```sh
just build                       # release build into bin/git-fs
claude plugin marketplace add ~/dev/claude-plugins/git-fs
claude plugin install git-fs@git-fs --scope user
```

Or `just install`, which does all three. A directory marketplace runs the
plugin from this folder, so a new `bin/git-fs` takes effect at the next
session start or `/reload-plugins`. The build tree goes to
`~/.cache/git-fs/target`, not into the checkout, because the install also
snapshots the whole directory. `just check` runs fmt, clippy and the 136
tests.

Standalone MCP config:

```json
{ "mcpServers": { "git-fs": { "command": "/path/to/git-fs", "args": ["mcp"] } } }
```

## Shell

```sh
git-fs call read '{"path":"src/main.rs","limit":40}'
git-fs call gitfs_ls
git-fs call gitfs_materialize '{"paths":["src/main.rs"]}'
git-fs tools
```

Each `call` is its own process, so a read in one call does not make the next
call's edit fresh; the MCP server keeps that state for the whole session.

## Environment

| Variable | Meaning |
| --- | --- |
| `GIT_FS_SESSION` | The session (branch name) to serve. Default: the one `session-start` pinned in `.git-fs/CURRENT`, else `main`. |
| `GIT_FS_DISK` | The workspace. Default: the directory the server was launched in. |
| `GIT_FS_CONFIG` | A JSON object laid over the defaults in `manifest.md` (`mode`, read and search caps, `integrate.*`, `store_dir`). |
| `GIT_FS_GATE_URL`, `GIT_FS_GATE_KEY` | An OpenAI-compatible endpoint `integrate` asks to review a diff when `integrate.gate_model` is set. No URL, no gate. |

`mode: "local"` writes the worktree directly; a workspace outside a Git
repository always behaves as local.

## Known ceilings

- Two Claude sessions in one checkout share `.git-fs/CURRENT`; the later
  `session-start` wins for both MCP servers. Set `GIT_FS_SESSION` per server to
  keep them apart.
- Native `Grep`, `Glob` and `Bash` see the disk, not the branch. Call
  `gitfs_materialize` before building or testing, or use the git-fs `grep`.
- File granularity, no rename tracking, no symlinks, no submodule content
  (a path inside a submodule is refused with the checkout to edit instead).

## Layout

| Path | Role |
| --- | --- |
| `src/main.rs` | `mcp`, `hook <action>`, `call <tool> [json]`, `tools`; answers as the pre-push hook when git runs it under that name |
| `src/mcp.rs` | stdio JSON-RPC server, requests in arrival order |
| `src/hooks.rs` | session-start, post-edit, read, stop |
| `src/store.rs` | the session branch over git plumbing, materialize ledger, catchup, integrate trees |
| `src/files.rs`, `src/search.rs` | bounded reads, guarded writes and edits, ripgrep stages |
| `src/overlay.rs`, `src/integrate.rs`, `src/push.rs` | the gitfs and integrate operations |
| `manifest.md` | the declared configuration defaults and operation contracts |
| `tests/unit/` | the suite, mounted by `#[path]` from each module |

## License

MIT
