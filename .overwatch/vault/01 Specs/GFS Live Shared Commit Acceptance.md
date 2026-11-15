---
title: GFS Live Shared Commit Acceptance
category: acceptance
status: implemented
proposals: []
issues: []
updated: 2026-11-15
tags: [spec]
checks: [cargo fmt --all -- --check, cargo clippy --all-targets --locked -- -D warnings, cargo test --locked, cargo build --release --locked]
paths: [src/store.rs, src/files.rs, src/overlay.rs, src/search.rs, src/hooks.rs, src/lib.rs, tests/unit/git_mode.rs, tests/tool_items.rs, README.md, docs/reference.md, install.txt, manifest.md]
proof: "7651c83886c8ae740ed65db72dcd90c153f12f4c guards: `cargo fmt --all -- --check` exit 0; `cargo clippy --all-targets --locked -- -D warnings` exit 0; `cargo test --locked` exit 0; `cargo build --release --locked` exit 0; limitations: Running git-fs MCP processes still use older installed executables and do not yet enumerate gitfs_snapshot; the compiled release exposes it. Safe runtime rollout remains unverified; active sessions were preserved."
evidence: 317fe562adf0e86d
---

# GFS Live Shared Commit Acceptance

## Summary

Deliver immediately peer-readable per-file Git commits on a repository-local live ref, safe conflict handling and rollback, isolated snapshots for native builds, and verified publication of the combined Windows implementation on main.

## Rules

-

## Plan


- [x] Publish tool edits and deletes to the live ref with optimistic conflict handling and retained session provenance
- [x] Expose shared live revisions in peer reads and searches, preserving deletions and safe rollback
- [x] Provide exact isolated worktree snapshots and remove stop-triggered shared materialization
- [x] Cover native disk capture, reviewed HEAD reconciliation and concurrent agent tests
- [x] Pass Windows format, Clippy, test, release and verify pushed main commit

## Status

Shared live Git commits now publish tool edits, deletions and captured native edits to a repository-local ref with peer-visible reads/searches, nonoverlapping merge, conflict refusal, rollback and exact isolated build snapshots. Windows format, strict Clippy, full tests and release build passed; reviewed main 7651c83886c8ae740ed65db72dcd90c153f12f4c was verified on origin/main.

## Open questions

-

## Related

- [[Home]]

## Sources

-
