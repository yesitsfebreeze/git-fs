# git-fs-ts spike — runbook

This file captures what's in the spike, what's deferred, and how to run the
perf gate.

## What's in

- `src/store.ts` — full port of `git-fs/src/store.rs` against isomorphic-git.
  All file ops (read/write/replace/patch/rm/ls), branch ops, log, diff, 3-way
  merge, checkout. Manual tree walking for merge since isomorphic-git does not
  ship a tree-merge primitive — instead we recurse base/ours/theirs, pick the
  side that diverged from base, and emit a conflict list otherwise.
- `src/mcp.ts` — stdio JSON-RPC server with verbatim copy of the Rust tool
  schemas, so existing skills/prompts keep working without edits.
- `src/cli.ts` — `init`, `branch *`, `write`, `read`, `rm`, `ls`, `merge`,
  `diff`, `log`, `checkout`, `show`.
- `bench/session-500.ts` — 500 writes + 500 replaces + 200 reads + merge +
  checkout. Prints per-phase wall-clock.

## What's deferred (port only if the gate passes)

- `git-fs hook *` subcommands (SessionStart/PostWrite/PostEdit/Read/Stop).
  These orchestrate the auto-commit + auto-merge flow. They depend on the
  Store surface, which is already ported — porting them is mechanical once
  the perf decision is made.
- `git-fs init-project` (writes `.mcp.json`). Mechanical port.
- `git-fs prune` (stale branch sweep). Mechanical port.
- Mergeignore-aware Stop merge (`load_mergeignore`, `strip_mergeignored`).
  Same pattern as Rust — read `.gitfs/mergeignore`, glob-strip from agent
  branch before merge.
- Sibling-overlap reconcile (`report_sibling_overlap`).

## Running the bench

```sh
# in repo root
cd git-fs-ts
npm install
npm run typecheck
npm run bench            # TS pass

# Rust comparator
cd ..
cargo build --release -p git-fs
hyperfine \
  --warmup 1 \
  'node git-fs-ts/dist/cli.js init --repo /tmp/rb && ...' \
  'target/release/git-fs --repo /tmp/rb init && ...'
```

The simpler approach: read the per-phase numbers `npm run bench` prints, then
re-run the same workload through `target/release/git-fs` and compare totals.

## Decision gate

| TS / Rust ratio | Action |
|-----------------|--------|
| ≤ 2×            | Commit to the port. Land hook subcommands. Cut a v2 release that drops the multi-target binary matrix and ships pure-JS via npm + plugin marketplace. |
| 2× – 5×         | Re-evaluate per-op breakdown. If write/replace are the long pole and unavoidable (object hashing in JS), keep Rust and instead shrink the binary (strip + LTO). |
| ≥ 5×            | Abort. Stay on Rust. |

## Measured (Windows 11, Node v24.14.1, both via MCP stdio)

| phase | TS dist/mcp.js | Rust git-fs-mcp | TS / Rust |
|-------|----------------|-----------------|-----------|
| writes (500)   | 2548 ms |  9075 ms | 0.28× |
| replaces (500) | 3421 ms | 11097 ms | 0.31× |
| reads (200)    |  334 ms |   215 ms | 1.55× |
| merge (1)      |    6 ms |    28 ms | 0.22× |
| checkout (1)   |  287 ms |   307 ms | 0.93× |
| **TOTAL**      | **6596 ms** | **20721 ms** | **0.32×** |

TS port is **~3× faster** than the Rust binary on this workload. Hypothesis:
libgit2 fsyncs object writes on each commit; isomorphic-git lets the OS
buffer them. The slower path is Windows-NTFS-specific — Linux numbers will
narrow the gap, but the spike already cleared the ≤2× bar by 6×, so the
gate is settled.

**Verdict: commit to the port.** Land the deferred hook subcommands,
mergeignore, sibling reconcile, and cut a v2 release that ships pure-JS
via npm. The Rust crate and the multi-target release workflow can be
deleted in the same change.

## Plugin wiring (when the gate passes)

Replace the `mcpServers.git-fs` entry in `.claude-plugin/plugin.json`:

```json
"mcpServers": {
  "git-fs": {
    "command": "node",
    "args": ["${CLAUDE_PLUGIN_ROOT}/git-fs-ts/dist/mcp.js"]
  }
}
```

`dist/launcher.js` (binary fetcher) becomes obsolete and gets deleted along
with the Rust crate and the release workflow.

## Known gaps in the spike implementation

- `Store.diff` emits a minimal unified-diff stub (whole-file -/+, no
  Myers/hunking). Good enough for `git_fs_diff` MCP smoke tests; the bench
  doesn't exercise it. Production port needs a real diff lib (e.g.
  `diff` npm package).
- `Store.listFiles` when `prefix` is non-empty re-writes the inner tree to
  get its OID. Cheap, but a follow-up could keep the OID without the rewrite.
- No `read_lines` UTF-8 line-ending parity test yet. Rust uses
  `text.lines()` which excludes trailing newline; the TS port mirrors that
  with an explicit `pop()`.
