# git-fs-ts (spike)

TypeScript port of [git-fs](../git-fs/). Pure-JS git backend via
[isomorphic-git](https://isomorphic-git.org/) — no native binary, no
multi-target release matrix, `npm install` is the entire installer.

## Status

Spike. **Not production-ready.** Tracks parity with Rust git-fs
v1.1.0. See `BENCH.md` for the perf gate decision.

## Layout

```
src/
  store.ts   — git object operations (open/init, branch_*, write/read/replace, merge, log, diff, checkout)
  mcp.ts     — stdio JSON-RPC MCP server (mirrors git-fs-mcp tool surface 1:1)
  cli.ts     — commander-style CLI (mirrors git-fs subcommands)
bench/
  session-500.ts — synthetic 500-commit session vs Rust binary
```

## Decision gate

Port is committed only if `npm run bench` lands within **2×** of the Rust
binary on the 500-commit synthetic session. ≥5× slower → revert.

## Surface parity

| Rust tool | TS export | Notes |
|-----------|-----------|-------|
| `git_fs_init` | `Store.init` | |
| `git_fs_branch_create` | `Store.branchCreate` | |
| `git_fs_branch_list` | `Store.branchList` | |
| `git_fs_branch_delete` | `Store.branchDelete` | |
| `git_fs_write` | `Store.writeFile` | |
| `git_fs_read` | `Store.readFile` / `readFileLines` | |
| `git_fs_patch` | `Store.patchFile` | |
| `git_fs_replace` | `Store.replaceFile` | |
| `git_fs_rm` | `Store.removeFile` | |
| `git_fs_ls` | `Store.listFiles` | |
| `git_fs_merge` | `Store.merge` | 3-way via isomorphic-git |
| `git_fs_diff` | `Store.diff` | unified text patch |
| `git_fs_log` | `Store.log` | |
| `git_fs_checkout` | `Store.checkout` | materialize to dest dir |
