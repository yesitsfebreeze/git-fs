---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B12 Guarded replica materialization"
paths:
  - "src/integrate.rs"
  - "src/store.rs"
  - "src/coordination.rs"
  - "tests/coordination_fencing.rs"
  - "tests/integration_recovery.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_fencing --test integration_recovery"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B13 Fenced integration and undo"
---
# B13 Fenced integration and undo

## Outcome

BC7: Integration and undo preserve exact candidate/review policy and reject stale delivery authority at the final ref transaction, retaining recoverable publication outcomes.

## Check

## Plan

- [ ] Bind preview, final candidate validation, integration and undo to the current delivery lease/fence acquired after unit_record releases execution ownership (Overwatch unit_load delivery:true); the old execution fence refuses.
- [ ] Reuse integration intent/reconciliation records and invalidate stale previews when source, authority or required evidence changes.
- [ ] Test revocation between preview/commit, moved main, lost acknowledgement and foreign disk/index preservation.
