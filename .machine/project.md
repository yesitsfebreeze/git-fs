# project.md — git-fs

**Name:** git-fs
**Domain:** Git session branches for AI agents. Every write through the git-fs tools is one commit on `refs/gitfs/<session>` in the project's own repository; reads and search see the branch first; materialize copies owned paths to disk under guards; hooks mirror native Claude edits onto the branch and materialize on Stop.
**Stack (v4):** Rust 2021, one binary `bin/git-fs` (`cargo build --release`, `just build`). Runtime needs `git` and `rg`. The Node v3 implementation is gone.
**Origin:** the `fs` kern of yesitsfebreeze/kern, ported without the kern host coupling (bridge, sessions attribution, router worker, buffer provider, context.file).
**Target:** a Claude Code plugin (`.claude-plugin/plugin.json` wires the MCP server, `hooks/hooks.json` the four hooks) and a standalone MCP server.

## Key paths
- `src/main.rs` — `mcp | hook <action> | call <tool> [json] | tools`; pre-push hook entry when invoked as `pre-push`.
- `src/mcp.rs` — stdio JSON-RPC, sequential dispatch; `src/hooks.rs` — the Claude hooks.
- `src/store.rs` — the core: session branch, materialize ledger, catchup, integrate trees.
- `manifest.md` — declared config defaults and gitfs/integrate contracts, embedded by `src/service.rs`.
- `tests/unit/` — mounted by `#[path]` from each module; `just check` = fmt, clippy, test.

## Commands
- `just build` / `just build-debug` — binary into `bin/`.
- `just check` — fmt, clippy (`-D warnings`), tests (136).
- `just install` — build, register this checkout as marketplace `git-fs`, install into the current Claude account.

## Status
v4.0.0: Rust port complete, suite green, installed from `~/dev/claude-plugins/git-fs`.
