---
kind: spec
prd: Live Shared Agent Commits
title: GFS Live Shared Commit Acceptance
status: migrated
depends: []
paths: []
checks: []
proof: "7651c83886c8ae740ed65db72dcd90c153f12f4c guards: `cargo fmt --all -- --check` exit 0; `cargo clippy --all-targets --locked -- -D warnings` exit 0; `cargo test --locked` exit 0; `cargo build --release --locked` exit 0; limitations: Running git-fs MCP processes still use older installed executables and do not yet enumerate gitfs_snapshot; the compiled release exposes it. Safe runtime rollout remains unverified; active sessions were preserved."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS Live Shared Commit Acceptance

## Outcome

Deliver immediately peer-readable per-file Git commits on a repository-local live ref, safe conflict handling and rollback, isolated snapshots for native builds, and verified publication of the combined Windows implementation on main.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Publish tool edits and deletes to the live ref with optimistic conflict handling and retained session provenance
- [x] Expose shared live revisions in peer reads and searches, preserving deletions and safe rollback
- [x] Provide exact isolated worktree snapshots and remove stop-triggered shared materialization
- [x] Cover native disk capture, reviewed HEAD reconciliation and concurrent agent tests
- [x] Pass Windows format, Clippy, test, release and verify pushed main commit

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS Live Shared Commit Acceptance.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS Live Shared Commit Acceptance
- category: acceptance
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-15
- tags: `spec`
- checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`
- paths: `src/store.rs`, `src/files.rs`, `src/overlay.rs`, `src/search.rs`, `src/hooks.rs`, `src/lib.rs`, `tests/unit/git_mode.rs`, `tests/tool_items.rs`, `README.md`, `docs/reference.md`, `install.txt`, `manifest.md`
- proof: 7651c83886c8ae740ed65db72dcd90c153f12f4c guards: `cargo fmt --all -- --check` exit 0; `cargo clippy --all-targets --locked -- -D warnings` exit 0; `cargo test --locked` exit 0; `cargo build --release --locked` exit 0; limitations: Running git-fs MCP processes still use older installed executables and do not yet enumerate gitfs_snapshot; the compiled release exposes it. Safe runtime rollout remains unverified; active sessions were preserved.
- evidence: 317fe562adf0e86d

### Status

Shared live Git commits now publish tool edits, deletions and captured native edits to a repository-local ref with peer-visible reads/searches, nonoverlapping merge, conflict refusal, rollback and exact isolated build snapshots. Windows format, strict Clippy, full tests and release build passed; reviewed main 7651c83886c8ae740ed65db72dcd90c153f12f4c was verified on origin/main.
