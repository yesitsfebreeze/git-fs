---
title: GFS-05 Honest Integration Outcomes and Recovery
category: behavior
status: target
proposals: []
issues: []
updated: 2026-11-14
tags: [spec]
depends: [GFS-04 Verification Receipts Bound to the Candidate]
checks: [cargo fmt --all -- --check, cargo test --locked --test integration_recovery --test guarded_push, cargo test --locked]
paths: [src/integrate.rs, src/store.rs, src/push.rs, manifest.md, tests/unit/integrate/tests.rs, tests/unit/store/tests.rs, tests/integration_recovery.rs, tests/guarded_push.rs, docs/reference.md]
---

# GFS-05 Honest Integration Outcomes and Recovery

## Summary

Distinguish durable Git publication from disk/index synchronization, including after restart. Extend the existing integration and sync routines, while preserving durable guarded-push attempts and uncertain outcomes.

## Rules

-

## Plan

- [ ] Define additive versioned outcomes for publication and worktree/index synchronization, including committed, synchronized, kept, failed and unknown with a bounded recommended next action.
- [ ] Make reconcile validate the original repository/session/branch/operation binding and report what can be proved after restart, HEAD advancement or branch change; do not rely on the current HEAD alone to identify an older integration.
- [ ] Persist sufficient recovery identity around the existing ref transaction and synchronization boundary; add an explicit guarded synchronization retry that never creates another commit or push.
- [ ] Use the existing synchronization routine to preserve foreign edits/staging and report kept paths; replay/reconcile must retain the same publication identity and never imply kept paths were synchronized.
- [ ] Add deterministic interruption tests before/after ref publication and during synchronization, including failed output persistence, changed HEAD and independently modified files/index.
- [ ] Extend existing guarded-push regressions for remote update with lost acknowledgement, same-ID replay, changed binding and intervening remote updates; retain unresolved attempts and reconcile authoritatively before another dispatch.

## Status

What exists today, with the ledger unit that owns the rest.

## Open questions

-

## Related

- [[Home]]

## Sources

-

## Acceptance examples

- Killing after ref publication cannot lead recovery to create a duplicate commit or misreport an unsynchronized checkout as synchronized.
- Foreign files and staging survive recovery; kept/error outcomes remain observable on later reconciliation.
- An uncertain remote effect remains unknown until proven, and an unrelated later remote update cannot be misattributed to this operation.

## Evidence and implementation limits

Current commit results expose worktree synced/kept/error, but replay/reconcile returns the Git receipt early without equivalent recovery state. Existing push identity, attempt persistence and reconciliation already exist and must be retained.

Scope is process-crash recovery and bounded schedules, not filesystem power-loss safety or globally exactly-once execution. Define retention/expiry semantics before any cleanup of resolved records; never garbage-collect an unresolved attempt.

## Research basis

RIFL, Crash-Testing, CHESS. See [[Git-FS Improvement Research 2026-11-14]].
