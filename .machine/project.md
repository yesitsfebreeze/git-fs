# project.md — git-fs

**Name:** git-fs
**Domain:** Copy-on-write overlay filesystem over the working directory, backed by a dedicated bare git object store. Each agent session edits on its own branch; only touched files become deltas; Stop materializes them to disk.
**Stack:** Plain ESM `.mjs`, Node ≥20. **No build step** (no transpile/bundle/output dir). **Zero runtime npm dependencies** — all object/ref/tree/merge ops shell out to the system `git` binary via `node:child_process`.
**Platform:** linux (WSL2 here); shell `nu`. Cross-platform Node CLI + MCP server.
**Target:** A Claude Code plugin / MCP server. `bin/git-fs.mjs` is the single entry; dispatches `cli | mcp | hook <action>` on `argv[2]`.

## Authoritative spec
The build spec lives in the session brief ("git-fs — overlay filesystem build spec", §1–§12). It is **normative** — pseudo code is to be followed to the T. Treat its principles D1–D4 and the §12 acceptance checklist as the contract.

## Key paths (planned per spec §4)
- `bin/git-fs.mjs` — single entry; dispatches `cli | mcp | hook <action>`.
- `src/store.mjs` — git wrapper + overlay primitives (**the core**; the `readFile` overlay-read is the choke point, spec §5).
- `src/mcp.mjs` — stdio JSON-RPC server (`Content-Length` framed), exports `run()`.
- `src/hooks.mjs` — session-start / post-edit / read / stop, exports `runHook()`.
- `test/store.test.mjs` — `node:test`.
- Store lives at `.git-fs/` (bare, gitignored, publishable). Override via `GIT_FS_REPO`.

## Commands
- **Test / quality gate:** `npm test` (= `node --test`; must pass with only Node installed — no build, no `node_modules`). NB: under Node ≥24 the bare-directory form `node --test test/` errors with `MODULE_NOT_FOUND`; use `npm test` or `node --test test/store.test.mjs`.
- **Build:** none (D1 — no build step).
- **Run MCP:** `node bin/git-fs.mjs mcp`
- **Run hook:** `node bin/git-fs.mjs hook <action>` (reads JSON payload on stdin)

## Build order (spec §11)
1. `src/store.mjs` + `test/store.test.mjs` green (empty-seed, disk-fallback read, write/replace/patch round-trip, materialize-touched-only incl. a `D` deletion, merge clean + conflict).
2. `src/mcp.mjs`, `src/hooks.mjs`, `bin/git-fs.mjs` dispatcher.
3. `package.json` + MCP/plugin config pointing at `bin/git-fs.mjs`.
4. End-to-end acceptance checks (spec §12).

## Status
Shipping — published Claude Code plugin, **v3.1.3** (`package.json` + `.claude-plugin/plugin.json` in lockstep, bumped via `just bump`/`just push`). The working dir **is** a git repo (branch `main`). All spec source exists and the suite is green (21 tests): `src/store.mjs` (core), `src/mcp.mjs`, `src/hooks.mjs`, `bin/git-fs.mjs` (single `cli | mcp | hook` entry), `test/store.test.mjs`. Recent: added `mergeLand` — a CAS-guarded recompute+land primitive (`store.mergeLand` / `git_fs_merge_land`) so concurrent landers can't drop each other's work via racy `update-ref`.
