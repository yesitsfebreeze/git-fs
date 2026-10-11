# Reference

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
- **Native tools steered by hooks.** `SessionStart` pins the session the MCP
  server serves and hands the model a short guide: which git-fs tool replaces
  `Read`, `Write`, `Edit`, `Grep`, `Glob`, `cat`, `sed -n`, `rg` and `find`,
  and when to materialize. `PreToolUse` enforces it inside the repository: a
  native `Write` or `Edit` is refused with the reason (use git-fs `edit` or
  `write` after a git-fs `read`); a native `Read`, `Grep`, `Glob` or a Bash
  call whose first command reads or writes files runs, with a note next to its
  result naming the git-fs tool that sees the branch. A `Read` of a path the
  branch changed gets that path onto disk first. A native `Write` or `Edit`
  that did run (mode `local`, or outside the repository) is mirrored onto the
  branch (`post-edit`); `Stop` materializes every owned path and reports what
  it left alone. Images, PDFs and notebooks pass to native `Read` untouched.

Session commits live under `refs/gitfs/*`. `.git-fs/` stores session indexes, read
records and the materialize ledger; git-fs adds it to `.git/info/exclude`.

## Codex integration

Codex selects `.codex-plugin/plugin.json`; Claude keeps its existing manifest.
Both hosts use the same Rust storage, freshness checks and materialization guards.
The Codex adapter provides:

- `SessionStart` guidance, including on resume and after compaction.
- `PreToolUse` denial of `apply_patch` touching repository paths, including
  multi-file patches, deletions and move destinations. Outside-repository patches
  and local mode pass through. Shell reads/writes receive the same advisory
  guidance as Claude; builds, tests and Git commands remain available.
- `PostToolUse` capture of files from native patches that ran in local mode.
- `Stop` and `SessionEnd` materialization of the session's owned paths, with
  conflict protection.
- Per-call MCP routing: the hook supplies the originating session and workspace,
  replacing any model-supplied routing. Concurrent Codex sessions do not share
  `CURRENT`, and the MCP process's launch directory does not select the workspace.
  Codex reports parent session IDs for subagent hooks, so those calls share the
  parent's git-fs branch.

The Codex MCP command is `git-fs mcp codex`. It refuses calls missing hook routing
with an actionable `/hooks` message instead of falling back to `main`. Use the
plugin for Codex rather than registering that command as a standalone MCP server.
The optional host tests exercise the installed Codex binary against a local,
deterministic model fixture, without an API key or a live model request:

```sh
python3 tests/codex_host.py
python3 tests/codex_host.py --code-mode
```

Hooks also apply to nested tool calls in Codex code mode. They are workflow
guardrails, not a sandbox for arbitrary shell programs.

Standalone MCP config:

```json
{ "mcpServers": { "git-fs": { "command": "/path/to/git-fs", "args": ["mcp"] } } }
```

## One or multiple items per tool

Every tool accepts its existing single-item arguments or an `items` array of
1–32 argument objects for that same tool. There is no separate `batch` tool.

Call `read` for either one file:

```json
{"path": "Cargo.toml"}
```

or several:

```json
{"items": [{"path": "Cargo.toml"}, {"path": "src/main.rs", "limit": 80}], "concurrency": 4}
```

After reading the files, call `edit` once for several edits:

```json
{"items": [
  {"path": "a.txt", "old": "before", "new": "after"},
  {"path": "b.txt", "edits": [{"old": "before", "new": "after"}]}
]}
```

Each item is one normal operation. `edit.edits` still applies several replacements
atomically within one file; `edit.items` performs separate edit operations,
including ordered edits to the same file. The whole call is not a transaction.

`gitfs_materialize` accepts `{"paths":["a.txt","b.txt"]}` for one operation on
several paths, or `{"items":[{"paths":["a.txt"]},{"paths":["b.txt"]}]}` for two
ordered operations. Integration, push, rollback, capture, inspection and all other
tools accept the same `items` form, using their usual argument objects.

Results stay in item order. Each entry contains `index` and the ordinary tool
`result` (`content`, `error`, and any `revision`). The outer content also reports
`failed` (executed items with errors) and `skipped`. The outer MCP `isError` is
true if an item failed. Single-item calls keep their original response shape;
even a one-element `items` array returns the multiple-item response.

`read`, `glob`, `grep` and `search` run independent items concurrently (four by
default; `concurrency` may be 1–8). They continue after individual errors.
Every other tool executes items sequentially and stops at the first error;
remaining entries have `skipped: true`. Successful earlier operations remain
applied. Inspect every result and retry only what is needed.

All items inherit the caller's workspace and session. Existing read-only peer
`session` selectors still work. Nested items, per-item routing overrides, mixing
single-item fields with `items`, and `concurrency` on mutations are refused before
any item executes. Ordinary argument/runtime errors belong to individual results.

Read freshness is retained on the same MCP connection. Await reads before edits.
Search refs/cursors and integration/push review revisions must come from an
earlier completed call; there is no result substitution between items. In
particular, a preview followed by a commit requires two calls, and a mutation
may invalidate a later item's previously reviewed revision. All normal guards
still apply. Multiple-item reads are independent views, not an atomic snapshot.

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

`mode: "local"` writes the worktree directly and turns the hook redirects off;
a workspace outside a Git repository always behaves as local.

## Known ceilings

- Two Claude sessions in one checkout share `.git-fs/CURRENT`; the later
  `session-start` wins for both MCP servers. Set `GIT_FS_SESSION` per server to
  keep them apart.
- Native `Grep`, `Glob` and `Bash` see the disk, not the branch; the hooks
  say so next to each result but do not block them. Call `gitfs_materialize`
  before building or testing, or use the git-fs `grep`.
- The `Write`/`Edit` refusal is per repository, not per path: a Claude session
  that cannot reach the git-fs MCP server has no way to write project files
  until it sets `GIT_FS_CONFIG='{"mode":"local"}'` or disables the plugin.
- File granularity, no rename tracking, no symlinks, no submodule content
  (a path inside a submodule is refused with the checkout to edit instead).

## Layout

| Path | Role |
| --- | --- |
| `src/main.rs` | `mcp`, `hook <session-start|pre-tool|post-edit|stop>`, `call <tool> [json]`, `tools`; answers as the pre-push hook when git runs it under that name |
| `src/mcp.rs` | stdio JSON-RPC server, requests in arrival order |
| `src/batch.rs` | shared per-tool items handling, bounded parallel reads, ordered mutations and per-item results |
| `src/hooks.rs` | shared session-start (guide), pre-tool (redirect, stale-read materialize), post-edit, stop |
| `src/codex.rs` | Codex patch targets, host guidance and per-call MCP session/workspace routing |
| `install/codex/` | Codex hook and MCP configuration |
| `src/store.rs` | the session branch over git plumbing, materialize ledger, catchup, integrate trees |
| `src/files.rs`, `src/search.rs` | bounded reads, guarded writes and edits, ripgrep stages |
| `src/overlay.rs`, `src/integrate.rs`, `src/push.rs` | the gitfs and integrate operations |
| `manifest.md` | the declared configuration defaults and operation contracts |
| `tests/unit/` | the suite, mounted by `#[path]` from each module |

## Development

`just check` runs formatting, Clippy and the Rust tests. `just build` places the
release binary in `bin/git-fs`, with build artifacts in `~/.cache/git-fs/target`.
