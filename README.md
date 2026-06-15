# git-fs

A copy-on-write overlay over the working directory, backed by a dedicated,
publishable bare git store. Plain ESM `.mjs`, Node ≥20, **no build step**, **zero
runtime npm dependencies** — all object/ref/tree/merge ops shell out to the
system `git` binary.

## Model
- The store is a bare repo at `.git-fs/` (gitignored, publishable as a subrepo).
- Each session works on `gitfs/<session-id>`, which **starts empty**.
- A file enters the branch only when an agent writes/edits it (touched-only deltas).
- **Reads fall back to the working tree** — touched paths read the branch blob,
  untouched paths read current disk bytes (no allowlist; gitignored files work).
- **Stop materializes only touched files** back to disk. The project's own `.git`
  is never written.

## Layout
- `bin/git-fs.mjs` — single entry; `argv[2]` selects `mcp | hook <action> | <cli>`.
- `src/store.mjs` — git wrapper + overlay primitives (the core; `readFile` is the choke point).
- `src/mcp.mjs` — hand-rolled stdio JSON-RPC MCP server.
- `src/hooks.mjs` — session-start / post-edit / read / stop.
- `test/store.test.mjs` — `node --test`.

## Usage
```
node bin/git-fs.mjs mcp                 # run the MCP server (stdio)
node bin/git-fs.mjs hook <action>       # session-start | post-edit | read | stop (JSON on stdin)
node bin/git-fs.mjs ls|read|log|init    # tiny CLI over the store
npm test                                # node --test
```
Env: `GIT_FS_REPO` (store dir, default `.git-fs`), `GIT_FS_DISK` (worktree,
default cwd), `GIT_FS_BRANCH` (default `main`).

As a Claude Code plugin: `.claude-plugin/plugin.json` wires the MCP server and the
four lifecycle hooks. Standalone MCP config: see `mcp.config.example.json`.
