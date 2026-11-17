---
kind: spec
prd: Reliable Concurrent Git-FS Contracts
title: GFS-05 Honest Integration Outcomes and Recovery
status: migrated
depends:
  - "Reliable Concurrent Git-FS Contracts/GFS-04 Verification Receipts Bound to the Candidate"
paths: []
checks: []
proof: "d499661d82d2c375832d3a4c0b0984a8f3ba823a guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test integration_recovery --test guarded_push` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); limitations: Legacy integrations without durable intent records cannot always be conclusively attributed; unknown publication outcomes fail closed, and unsafe branch or HEAD states require returning to the verified source before synchronization."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-05 Honest Integration Outcomes and Recovery

## Outcome

Distinguish durable Git publication from disk/index synchronization, including after restart. Extend the existing integration and sync routines, while preserving durable guarded-push attempts and uncertain outcomes.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Define additive versioned outcomes for publication and worktree/index synchronization, including committed, synchronized, kept, failed and unknown with a bounded recommended next action.
- [x] Make reconcile validate the original repository/session/branch/operation binding and report what can be proved after restart, HEAD advancement or branch change; do not rely on the current HEAD alone to identify an older integration.
- [x] Persist sufficient recovery identity around the existing ref transaction and synchronization boundary; add an explicit guarded synchronization retry that never creates another commit or push.
- [x] Use the existing synchronization routine to preserve foreign edits/staging and report kept paths; replay/reconcile must retain the same publication identity and never imply kept paths were synchronized.
- [x] Add deterministic interruption tests before/after ref publication and during synchronization, including failed output persistence, changed HEAD and independently modified files/index.
- [x] Extend existing guarded-push regressions for remote update with lost acknowledgement, same-ID replay, changed binding and intervening remote updates; retain unresolved attempts and reconcile authoritatively before another dispatch.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-05 Honest Integration Outcomes and Recovery.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-05 Honest Integration Outcomes and Recovery
- category: behavior
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-16
- tags: `spec`
- depends: `GFS-04 Verification Receipts Bound to the Candidate`
- checks: `cargo fmt --all -- --check`, `cargo test --locked --test integration_recovery --test guarded_push`, `cargo test --locked`
- paths: `src/integrate.rs`, `src/store.rs`, `src/push.rs`, `manifest.md`, `tests/unit/integrate/tests.rs`, `tests/unit/store/tests.rs`, `tests/integration_recovery.rs`, `tests/guarded_push.rs`, `docs/reference.md`
- proof: d499661d82d2c375832d3a4c0b0984a8f3ba823a guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test integration_recovery --test guarded_push` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); limitations: Legacy integrations without durable intent records cannot always be conclusively attributed; unknown publication outcomes fail closed, and unsafe branch or HEAD states require returning to the verified source before synchronization.
- evidence: 4aa011b9ef6c7eea

### Status


Versioned integration outcomes and durable original-ref recovery now preserve exact commit identity across restart and branch changes; guarded retry synchronizes only when the original branch and HEAD are safe, preserving foreign edits and existing push replay safety. Native MCP, race, crash-window, guarded-push and full Rust tests pass at d499661.

### Acceptance examples


- Killing after ref publication cannot lead recovery to create a duplicate commit or misreport an unsynchronized checkout as synchronized.
- Foreign files and staging survive recovery; kept/error outcomes remain observable on later reconciliation.
- An uncertain remote effect remains unknown until proven, and an unrelated later remote update cannot be misattributed to this operation.

### Evidence and implementation limits


Current commit results expose worktree synced/kept/error, but replay/reconcile returns the Git receipt early without equivalent recovery state. Existing push identity, attempt persistence and reconciliation already exist and must be retained.

Scope is process-crash recovery and bounded schedules, not filesystem power-loss safety or globally exactly-once execution. Define retention/expiry semantics before any cleanup of resolved records; never garbage-collect an unresolved attempt.

### Research basis


RIFL, Crash-Testing, CHESS. See [[Git-FS Improvement Research 2026-11-14]].
