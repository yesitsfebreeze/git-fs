# Reference

## What you get

- **Guarded writes.** `write`, `edit` and `delete` are refused unless the bytes the
  session last read are still current. A whole-file read of more than 350 lines
  is refused; read a range or grep. `edit` replaces one exact unique match, all
  matches with `all`, or an `edits` array in one all-or-nothing commit. When a
  sibling agent's committed revision (on trunk or its session branch) reaches
  the shared worktree after this session's read, `edit` rebases onto it if its
  `old` text is still unique; an unknown hand edit is still refused.
- **Live shared reads and search.** `read`, `glob`, `grep` and `search`
  (typed ripgrep stages with cursors) see `refs/gitfs-live/current` before
  the session branch and worktree. Every tool-mediated successful edit or delete
  publishes a shared commit without updating `HEAD`, index, or disk. Explicit
  deletion tombstones hide recreated files until reconciled.
- **The session branch.** `gitfs_ls`, `gitfs_diff`, `gitfs_rollback` (one
  commit, or one path back to session start), `gitfs_catchup` (rebase onto
  moved trunk after checking which owned paths trunk changed under you;
  non-overlapping changes to one file from two sessions merge three-way),
  `gitfs_materialize` (copy owned paths to disk, refusing files newer on disk or
  modified since the last materialize, `force` to override; a copy another
  session committed and materialized is merged instead of refused),
  `gitfs_capture` (pull disk changes back in; explicit paths can adopt
  additions, edits and tracked deletions, while omitted paths stay limited to
  owned paths), and `gitfs_snapshot` (create a detached checkout pinned to
  the shared commit, or release it with its returned `id`). Materialization
  and catchup report the paths they merged under `merged`.
- **Parallel edits.** Each successful change advances one shared ref using a
  compare-and-swap update. Disjoint file edits converge; non-overlapping text
  changes merge three-way; overlapping changes refuse with the path named.
  A stale session branch cannot silently overwrite its last published state.
  When reviewed `HEAD` advances, non-conflicting changes are merged into live
  as a two-parent commit; a real conflict requires manual reconciliation.
- **Publication.** `integrate` previews and commits the session onto trunk
  with a secret scan, `push` runs a guarded push with `operation_id`
  idempotency and a private pre-push hook that also runs the repository's
  existing pre-push hooks once, `undo` reverts the last integrated
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
  branch (`post-edit`); `Stop` leaves the shared checkout unchanged. Use `gitfs_snapshot`
  for builds or explicit `gitfs_materialize` when needed. Images, PDFs and notebooks pass to native `Read` untouched.

Session commits live under `refs/gitfs/*`; the latest locally shared source is
`refs/gitfs-live/current`. `.git-fs/` holds session indexes, read records,
materialization metadata and generated snapshot checkouts, excluded from Git.
The live ref is local: remote publication still requires reviewed integration
and the guarded push path.

A project without `.git` needs none: the first write makes `.git-fs/` a git
directory of its own that holds only the session branches and their objects.
Every path a session writes counts as added, since there is no trunk to
compare with; `integrate`, `gitfs_catchup` and `push` refuse for the same
reason, and `gitfs_materialize` is how changes reach the worktree.

## Isolated build and test snapshots

Call `gitfs_snapshot {}` to create a clean detached worktree at the exact live
commit. Its result includes `id`, absolute `path`, `revision` and `head`.
Run the build in that path instead of materializing another agent's work into
the shared checkout. The snapshot stays pinned if new edits arrive.

Call `gitfs_snapshot {"id":"<returned id>"}` after testing. Only the same
session can release its snapshot; Git refuses removal while it has tracked or
untracked changes. Generated ignored build outputs may be removed with the
checkout. Snapshots require a normal project Git repository (not a standalone
`.git-fs` object store).

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
- `Stop` and `SessionEnd` keep the shared checkout untouched; use a pinned
  `gitfs_snapshot` checkout or explicit materialization for native execution.
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

## Deletions and changes made outside git-fs

Read a file before calling `delete`, just as before `write` or `edit`:

```json
{"path": "obsolete.txt"}
```

Pass that argument to `read`, then to `delete` on the same MCP connection. In
git mode, deletion is recorded on the session branch and takes effect on disk
when materialized. It is distinct from writing an empty file. Reads and searches
respect the deletion, and rollback can restore the prior content. In local
mode, `delete` removes the file from disk directly.
Git-mode deletion needs a file tracked by trunk or the session; use local mode
for an untracked file that exists only on disk.

To bring selected changes made by a formatter, editor or local-mode tool into
a session branch, call `gitfs_capture` with explicit paths:

```json
{"paths": ["new.txt", "edited.txt", "obsolete.txt"]}
```

Present files are captured with their current bytes; a selected tracked file
that is missing from disk is captured as a deletion. A missing path with no
tracked or session preimage is an error. Permission errors and other read
failures are never treated as deletions. Explicit selection can start a new
session branch and leaves shared staging untouched. Omit `paths` to capture
only session-owned files; `paths: []` captures nothing. Review `gitfs_diff`
before integrating the resulting session.

## Shell

```sh
git-fs call read '{"path":"src/main.rs","limit":40}'
git-fs call gitfs_ls
git-fs call gitfs_materialize '{"paths":["src/main.rs"]}'
git-fs tools
```

Each `call` is its own process, so a read in one call does not make the next
call's edit fresh; the MCP server keeps that state for the whole session.

## Publication completion and handoff

Saving a session branch, materializing files, integrating a trunk commit and
pushing a remote ref are separate outcomes. Stop leaves disk untouched. Integration
returns `push: false`. Finish a requested delivery by pushing the exact integrated
commit and observing a confirmed remote receipt.

An originating session can authorize a different session to finish one reviewed
push without giving it the originating identity:

1. The source calls `push` with `phase: "preview"`, the exact `remote`, `ref`,
   `commit`, `expected_remote_head`, and `recipient_session`.
2. It repeats those fields with `phase: "handoff"`, the preview's
   `expected_revision`, and a distinct `operation_id`. This stores a durable
   `ready` receipt; it does not push.
3. The recipient, under its own trusted session, calls
   `push {phase: "reconcile", operation_id}`. The receipt names the original
   source in `binding.authorized_by` and the recipient in `binding.session`.
4. The recipient sends `phase: "commit"`, the same `operation_id`,
   `expected_revision: receipt.revision`, and the receipt's `remote`,
   `ref: binding.reference`, `commit` and `expected_remote_head`.
   It omits `recipient_session`.

The grant requires the source's terminal integration receipt and the exact current
local destination ref. Use the same repository and worktree after restart. A
changed local destination, repository, remote URL, expected remote head or current
review policy blocks dispatch. Configured pre-push hooks still run. Concurrent
recipient retries persist `unknown` before dispatch and cannot dispatch the same
operation twice. Reconcile an uncertain outcome; never create a fresh operation
merely because an acknowledgement was lost.

Handoffs share the bounded push-attempt store and its retention rules. A
`ready` receipt means authorized but not dispatched; `unknown` remains unresolved;
`confirmed` requires the exact remote commit to have been observed. Reconciliation
can confirm an already-delivered grant without another push. A late ready-state
readback cannot make an already-dispatched operation dispatchable again.

Ordinary pushes remain limited to commits integrated by the current session.
A worker that ended before creating a grant has not authorized a replacement
session; restarting with its identity or supplying another session name is not a
handoff. Resume a genuinely continuous session with its stable identity, or arrange
the source-authorized handoff before it ends.

## Environment

| Variable | Meaning |
| --- | --- |
| `GIT_FS_SESSION` | The session (branch name) to serve. Default: the one `session-start` pinned in `.git-fs/CURRENT`, else `main`. |
| `GIT_FS_DISK` | The workspace. Default: the directory the server was launched in. |
| `GIT_FS_CONFIG` | A JSON object laid over the defaults in `manifest.md` (`mode`, read and search caps, `integrate.*`, `store_dir`). |
| `GIT_FS_GATE_URL`, `GIT_FS_GATE_KEY` | An OpenAI-compatible endpoint `integrate` asks to review a diff when `integrate.gate_model` is set. A configured model with a missing endpoint or key blocks integration. |

`mode: "local"` writes the worktree directly and turns the hook redirects off.
A workspace outside a Git repository stays in git mode: its first git-fs write
makes the store directory a Git directory of its own.

## Repository review gate

Set a command in **local Git configuration** to require an independent review
before this repository's session changes integrate. Other repositories keep
their existing behavior. The command is an argv array, executed directly without
a shell; use absolute executable and script paths:

```sh
git config --local gitfs.reviewCommand '["/usr/bin/python3","/absolute/git-fs/scripts/codex-review.py","/absolute/bin/codex"]'
git config --local gitfs.reviewTimeoutMs 300000
```

The command receives one JSON object on stdin:
`{schema:"gitfs-review-request/v1", repo, session, base, tree, review_revision,
diff, intent}`. It returns one JSON object on stdout:
`{approved:boolean, summary:string, findings:[{id,severity,path,line,evidence}]}`.
Finding IDs must be unique; paths are repository-relative and lines start at 1.
`blocker`, `high` and `medium` findings block approval; `low` is advisory. A failed
command, timeout, malformed response or incomplete input cannot approve a change.
Timeout defaults to five minutes, capped at one hour; diff input is capped at
4 MiB and each output stream at 128 KiB. The bundled Codex runner reviews an
isolated candidate snapshot and loads review guidance from the base revision.

Use the optional third argument to pin the Codex executable as part of the
policy. The preview binds the candidate and base, repository identity, session, command
argv, timeout and the bytes of absolute file arguments (including the executable
and reviewer script). Fingerprints stream files up to 512 MiB each. Changing any
of those requires another preview and review.
Policy and candidate are rechecked before publication. Review verdicts and
bindings stay in `.git/gitfs-review/`; no report is added to source control.
The review is code inspection and does **not** attest that tests were executed.

Every newly pushed commit must have a matching approved receipt under current
policy. Git-fs checks before push and again in its private pre-push hook. A native
Git pre-push hook can call `git-fs review-check <full-commit-oid> --repo <path>`
for each outgoing commit. Native commits without integration receipts are held;
use the session integration workflow. Initial pushes or pending older commits
without receipts need an explicit migration plan; enabling the gate does not
retroactively approve them. A push is limited to 256 new commits.

After three held verdicts in one session, review stops for reassessment.
Infrastructure errors do not consume rounds. After reconsidering the approach,
start another cycle with `git-fs review-reset <session> --reason '<what changed>'
--repo <path>`; the reason and held revisions remain in local history. Successful
integration review clears the consecutive hold count. The gate is a local
workflow control: a process allowed to edit `.git` or bypass hooks can change
policy or evidence. It is not an adversarial sandbox or a replacement for remote
branch protection.

Run `just check` for the Rust policy, integration and private push-hook checks;
run `python3 -m unittest discover -s tests -p codex_review_test.py` for the bundled
reviewer's snapshot and response validation checks.

## Known ceilings

- Session pins are written per ancestor process id, so two hosts in one
  checkout keep their own sessions. A host whose hook and MCP server share no
  ancestor, or whose process ids were reused after a crash, falls back to
  `.git-fs/CURRENT`; set `GIT_FS_SESSION` per server there to keep them apart.
- A merge is text only. A binary file both sessions changed, or changes to the
  same lines, keeps the refusal; read the path again and rewrite it.
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
| `src/merge.rs` | three-way text merge for a path two sessions changed |
| `src/session_pin.rs` | per-ancestor session pins and MCP lookup |
| `src/files.rs`, `src/search.rs` | bounded reads, guarded writes and edits, ripgrep stages |
| `src/overlay.rs`, `src/integrate.rs`, `src/push.rs` | the gitfs and integrate operations |
| `manifest.md` | the declared configuration defaults and operation contracts |
| `tests/unit/` | the suite, mounted by `#[path]` from each module |

## Development

`just check` runs formatting, Clippy and the Rust tests. `just build` places the
release binary in `bin/git-fs`, with build artifacts in `~/.cache/git-fs/target`.
