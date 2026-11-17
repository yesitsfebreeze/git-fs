---
kind: spec
prd: Reliable Concurrent Git-FS Contracts
title: GFS-02 Stable Session and Workspace Binding
status: migrated
depends:
  - "Reliable Concurrent Git-FS Contracts/GFS-01 Runtime Identity and Protocol Negotiation"
paths: []
checks: []
proof: "35efd597423f1b7c5efd6978fce022566316a17a guards: `cargo fmt --all -- --check` exit 0; `cargo test --locked --test session_routing` exit 0; `cargo test --locked` exit 0; limitations: The stdio protocol alone cannot authenticate model supplied Codex routing or distinguish unrelated clients multiplexed through one generic backend. Host hooks must overwrite Codex context, and the Tailscale gateway must isolate or authenticate each client. GIT_FS_COORDINATED=1 remains opt-in; deployment and GFS03-GFS06 are separate."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-02 Stable Session and Workspace Binding

## Outcome

Prevent a coordinated caller from reading under one workspace/session identity and mutating under another. Preserve trusted host-supplied Codex routing and add expectations that validate identity rather than allowing a model to choose a different target.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Specify a coordinated-call identity expectation derived from GFS-01; validate root/store/session binding before any mutating operation, including capture, materialize, integrate and push.
- [x] Bind freshness records and the connection or trusted per-call routing context to the validated identity; a changed ancestor pin or CURRENT value must refuse stale mutations instead of redirecting them.
- [x] Require an explicit or trusted host-resolved distinct identity in coordinated mode; retain any legacy fallback only through a documented compatibility policy that exposes its weaker guarantees.
- [x] Test two real clients, explicit environment sessions, ancestor pins, a late SessionStart, changed CURRENT, nested roots, resume/reconnect and a mismatched expectation.
- [x] Retain Codex's authority over injected routing and reject model attempts to replace it; document host/hub obligations and the recovery action for an identity mismatch.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-02 Stable Session and Workspace Binding.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-02 Stable Session and Workspace Binding
- category: behavior
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-15
- tags: `spec`
- depends: `GFS-01 Runtime Identity and Protocol Negotiation`
- checks: `cargo fmt --all -- --check`, `cargo test --locked --test session_routing`, `cargo test --locked`
- paths: `src/mcp.rs`, `src/session_pin.rs`, `src/codex.rs`, `src/service.rs`, `tests/unit/session_pin.rs`, `tests/session_routing.rs`, `docs/session-routing.md`
- proof: 35efd597423f1b7c5efd6978fce022566316a17a guards: `cargo fmt --all -- --check` exit 0; `cargo test --locked --test session_routing` exit 0; `cargo test --locked` exit 0; limitations: The stdio protocol alone cannot authenticate model supplied Codex routing or distinguish unrelated clients multiplexed through one generic backend. Host hooks must overwrite Codex context, and the Tailscale gateway must isolate or authenticate each client. GIT_FS_COORDINATED=1 remains opt-in; deployment and GFS03-GFS06 are separate.
- evidence: 0317acf73d7f1689

### Status


MCP standard clients now freeze canonical workspace, store, session and source on first tool call; Codex keeps independent per-hook-session bindings. Drift fails before read or mutation, including capture. Coordinated mode requires explicit session or ancestry pin. Seven native routing regressions and all integrated Cargo guards passed. Published main 35efd597423f1b7c5efd6978fce022566316a17a.

### Acceptance examples


- A read under session A followed by a pin/root change cannot mutate session B.
- Independent coordinated clients have distinct identities, while valid trusted Codex routing continues to work.
- A refusal names the mismatched binding without exposing credentials or silently creating another session.

### Evidence and implementation limits


Non-Codex session resolution currently rereads environment/pins/CURRENT/default main each call. This is a routing risk observed in source; an unintended cross-session write was not reproduced.

This unit and GFS-03 can run in parallel after GFS-01 because their declared source paths do not overlap. Recheck actual paths and live claims before assigning both.

### Research basis


MCP-Lifecycle. See [[Git-FS Improvement Research 2026-11-14]].
