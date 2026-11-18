---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B05 Fenced file publication"
paths:
  - "src/store.rs"
  - "src/overlay.rs"
  - "src/service.rs"
  - "tests/coordination_fencing.rs"
  - "tests/coordination_recovery.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_fencing --test coordination_recovery"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B06 Capture and rollback fencing"
---
# B06 Capture and rollback fencing

## Outcome

BC7: Capture and rollback cannot bypass coordinated source fencing or overwrite foreign work, and their partial effects use the same recoverable outcome contract.

## Check

## Plan

- [ ] Apply trusted mutation context to explicit capture selection and path/commit rollback.
- [ ] Preserve deletion tombstones, known-peer merge and ownership behavior while fencing each accepted source transition.
- [ ] Test stale/revoked capture and rollback, partial batches and restart reconciliation with actual peer-history and dirty-file assertions.
