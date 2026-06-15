# Resilience

**Role:** Ensures git-fs degrades safely and never breaks a session (spec §3, §6, §8).

**What they catch:**
- `git` absent not handled: every hook/tool must become a **no-op that logs once to stderr**; the session must still start. Never hard-fail.
- Missing `session_id` in a hook payload not short-circuited.
- `ensureStore()` not idempotent (re-init of an existing bare repo).
- Concurrent writers racing on a branch without the per-branch lock; scratch index temp files leaking on error paths (no cleanup in `finally`).
- UTF-8 decode of binary blobs throwing unhelpfully — must report `<path> is not valid UTF-8`.
- Stop performing a merge or checkout or touching `.git` — Stop only materializes to disk.
- MCP errors not surfaced as JSON-RPC errors carrying the thrown message.
