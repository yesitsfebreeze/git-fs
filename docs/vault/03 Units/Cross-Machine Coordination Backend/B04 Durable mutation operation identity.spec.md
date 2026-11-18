---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B03 Durable fence installation"
paths:
  - "src/coordination.rs"
  - "src/provenance.rs"
  - "tests/coordination_recovery.rs"
  - "docs/coordination.md"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_recovery"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B04 Durable mutation operation identity"
---
# B04 Durable mutation operation identity

## Outcome

BC3: Caller-bound durable intent and outcome records survive restart and prevent changed-intent reuse or automatic redispatch after uncertain effects.

## Check

## Plan

- [ ] Persist stable operation id, canonical intent digest, binding, fence and expected revisions before non-repeatable effects using existing store conventions.
- [ ] Implement inspection/reconcile for not-dispatched, refused, session-only, shared-accepted and unknown outcomes, retaining original target evidence.
- [ ] Test duplicate/changed payload and foreign-session ids, corrupt receipts and retention expiry; consumed or unresolved ids never become dispatchable again.
