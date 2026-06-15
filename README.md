<div align="center">

# 🗂️ git-fs

**git worktrees for AI agents — invisible, disposable, and dependency-free.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Node](https://img.shields.io/badge/node-%E2%89%A520-339933?logo=node.js&logoColor=white)](https://nodejs.org)
[![Zero deps](https://img.shields.io/badge/runtime%20deps-0-brightgreen)](package.json)
[![No build](https://img.shields.io/badge/build%20step-none-brightgreen)](package.json)
[![MCP](https://img.shields.io/badge/MCP-server-7C3AED)](https://modelcontextprotocol.io)
[![Version](https://img.shields.io/badge/version-3.0.2-informational)](package.json)
[![Stars](https://img.shields.io/github/stars/yesitsfebreeze/git-fs?style=social)](https://github.com/yesitsfebreeze/git-fs/stargazers)

</div>

---

A copy-on-write overlay over your working directory, backed by a dedicated bare
git store. Agents edit inside the overlay; nothing touches your real files until
you accept it — and even then, only the files they actually changed.

Plain ESM `.mjs`, Node ≥20. **No build step. Zero runtime npm dependencies** —
every object/ref/tree/merge op shells out to the system `git` binary.

## Why

Letting an agent loose on a repo usually means trusting it not to scribble
everywhere. git-fs removes the trust: edits land in a per-session git branch, you
see exactly what changed, and you merge it in — or throw it away — on your terms.

- 🛡️ **Sandbox agents.** Edits live in an overlay branch. Your real files only
  change on `stop`, and only the touched ones. Don't like it? Delete the branch.
- 🌿 **Run agents in parallel.** Each session is its own branch in one shared
  store. Fan out N agents on one checkout, then `merge` the winners — clean
  merges auto-commit, conflicts come back as a file list.
- 🧪 **Speculate freely.** "Try this refactor, show me the diff, toss it" —
  without touching disk or polluting your project's git history.
- 📂 **Edit anywhere.** Reads fall back to live disk, so gitignored files and
  non-git directories work transparently.
- 🧾 **Audit everything.** Every edit is a commit tagged with its `Session-Id`.
  Per-session, per-file history of exactly what an agent did, in order.

## How it works

```

   agent writes ───▶  overlay branch  gitfs/<session-id>
                      (starts EMPTY — touched files only)                   
                                         │
   agent reads ──▶  tracked? ──▶ YES ──▶ branch blob (read-your-writes)
                       │
                       └──────▶ NO  ──▶  live disk bytes (fallback)
                                         │
   on stop ─────────────────────────────▼─────────────────────
                    materialize touched files ──▶  working dir
                    (your project's .git is NEVER written)
```

- The store is a bare repo at `.git-fs/` (gitignored, publishable as a subrepo).
  Your project's own `.git` is **never** written.
- Each session works on `gitfs/<session-id>`, which **starts empty**.
- A file enters the branch only when an agent writes/edits it (touched-only deltas).
- **Reads fall back to the working tree** — touched paths read the branch blob
  (read-your-writes), untouched paths read current disk bytes. No allowlist.
- **Stop materializes only touched files** back to disk.

Three-way merges run in memory via `git merge-tree --write-tree` — no worktree,
no checkout. git-fs never hard-fails a session: no `git` on PATH ⇒ silent no-op.

## Layout

| File | Role |
| --- | --- |
| `bin/git-fs.mjs` | single entry; `argv[2]` selects `mcp \| hook <action> \| <cli>` |
| `src/store.mjs` | git wrapper + overlay primitives (the core; `readFile` is the choke point) |
| `src/mcp.mjs` | hand-rolled stdio JSON-RPC MCP server |
| `src/hooks.mjs` | session-start / post-edit / read / stop |
| `test/store.test.mjs` | `node --test` |

## Usage

```sh
node bin/git-fs.mjs mcp                 # run the MCP server (stdio)
node bin/git-fs.mjs hook <action>       # session-start | post-edit | read | stop (JSON on stdin)
node bin/git-fs.mjs ls|read|log|init    # tiny CLI over the store
npm test                                # node --test
```

Env: `GIT_FS_REPO` (store dir, default `.git-fs`), `GIT_FS_DISK` (worktree,
default cwd), `GIT_FS_BRANCH` (override the branch). With no override, the MCP
server and CLI default to the branch `session-start` pinned for the active
session (`.git-fs/CURRENT`), falling back to `main` only outside a session.

As a Claude Code plugin, `.claude-plugin/plugin.json` wires the MCP server and the
four lifecycle hooks. For a standalone MCP config, see `mcp.config.example.json`.

## Good to know

- File-granularity, blob+path — no rename/move tracking, no symlinks.
- `materialize` writes branch blobs to disk, but refuses to overwrite a path
  whose on-disk copy is newer than the blob (the commit that produced it) and
  reports it as a conflict; pass `force` to override.
- The write lock is a lightweight per-branch spin lock — sized for a handful of
  concurrent agents, not a high-throughput server.

## Star map

[![Star History Chart](https://api.star-history.com/svg?repos=yesitsfebreeze/git-fs&type=Date)](https://star-history.com/#yesitsfebreeze/git-fs&Date)

<div align="center">

If git-fs saved you from an agent gone rogue, drop a ⭐ — it helps.

</div>

## License

MIT
