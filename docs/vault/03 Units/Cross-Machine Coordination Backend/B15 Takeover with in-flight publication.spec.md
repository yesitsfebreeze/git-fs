---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B14 Fenced push dispatch"
paths:
  - "src/coordination.rs"
  - "src/push.rs"
  - "src/store.rs"
  - "tests/coordination_fencing.rs"
  - "tests/coordination_recovery.rs"
  - "tests/guarded_push.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_fencing --test coordination_recovery --test guarded_push"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B15 Takeover with in-flight publication"
---
# B15 Takeover with in-flight publication

## Outcome

BC2/BC7: Takeover remains pending until unsettled effects within the reassigned scope are quiescent or reconciled; unrelated source work, replicas and builds continue.

## Check

## Plan

- [ ] Consult durable source/delivery outcomes at fence advance; a completed live CAS with a missing hint is settled, while an admitted uncertain push remains unresolved.
- [ ] Use existing destination-scoped push reconciliation and owned-process lifecycle before acknowledging takeover without holding a repository lock through network delay or killing peers.
- [ ] Inject failures before dispatch, after dispatch, after remote acceptance and before receipt update; assert actual destination ref, original receipt, no duplicate push and concurrent unrelated work.
