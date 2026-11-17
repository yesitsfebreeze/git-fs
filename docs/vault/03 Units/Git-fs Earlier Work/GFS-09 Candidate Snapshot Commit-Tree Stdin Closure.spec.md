---
kind: spec
prd: Git-fs Earlier Work
title: GFS-09 Candidate Snapshot Commit-Tree Stdin Closure
status: migrated
depends:
  - "Reliable Concurrent Git-FS Contracts/GFS-03 Exact Integration Candidate Snapshots"
paths: []
checks: []
proof: "426b7be368db0694e017b9478b813e59ed5a795e guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test candidate_snapshot_stdio` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: none"
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-09 Candidate Snapshot Commit-Tree Stdin Closure

## Outcome

Native git-fs MCP retains an open stdin for JSON-RPC. Candidate snapshot creation in src/snapshot.rs invokes `git commit-tree` without a message argument, so Git inherits the open MCP pipe, waits for EOF and hangs indefinitely. Reproduce with a bounded child whose stdin stays open; supply a deterministic commit message without reading stdin, prove candidate commit/tree and checkout/index preservation, and publish guarded proof independently of the separate GFS-08 MCP schema repair.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Verify current FEB canonical checkout, overlapping claims, and reproduce the open-stdin candidate commit-tree hang with a bounded native test before production edits.
- [x] Add a deterministic message to the candidate Git commit-tree subprocess so persistent MCP stdin is never read; preserve tree/parent/author and error guards.
- [x] Add an isolated native regression with an explicitly open stdin and a bounded deadline that proves no hang, no duplicate ref publication, exact candidate tree and unchanged shared HEAD/index.
- [x] Run focused red-to-green regression, formatting, strict all-target Clippy, full locked Rust suite and release build against the exact isolated source.
- [x] Integrate only the owned snapshot implementation/regression under normal Git hooks, fast-forward current main, push non-force and independently verify remote SHA.
- [x] Record all planned tasks and passing configured command guards in Overwatch, publish the exact unit_record receipt and preserve all unrelated vault metadata.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-09 Candidate Snapshot Commit-Tree Stdin Closure.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-09 Candidate Snapshot Commit-Tree Stdin Closure
- category: behavior
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-16
- tags: `spec`
- depends: `GFS-03 Exact Integration Candidate Snapshots`
- checks: `cargo fmt --all -- --check`, `cargo test --locked --test candidate_snapshot_stdio`, `cargo test --locked`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo build --release --locked`
- paths: `src/snapshot.rs`, `tests/candidate_snapshot_stdio.rs`
- proof: 426b7be368db0694e017b9478b813e59ed5a795e guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test candidate_snapshot_stdio` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: none
- evidence: 1b91c05129fdd94a

### Status

GFS-09 fixed candidate snapshot hangs in long-lived MCP processes: commit-tree now receives deterministic -m message instead of reading inherited stdin. Native child-process test retained open stdin and failed at its bounded deadline before fix, then passed; checks exact reviewed tree, candidate subject and blob, release, unchanged shared HEAD/index/status, unchanged baseline refs and only the owned session ref. Isolated and canonical all five required Rust guards passed. Source commit 426b7be368db0694e017b9478b813e59ed5a795e was normal-hook committed, ff-only integrated, non-force pushed and independently confirmed on GitHub main.
