---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B06 Capture and rollback fencing"
paths:
  - "src/store.rs"
  - "src/service.rs"
  - "src/inspection.rs"
  - "tests/coordination_fencing.rs"
  - "tests/coordination_contract.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_fencing --test coordination_contract"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B07 Reconciliation side-effect coverage"
---
# B07 Reconciliation side-effect coverage

## Outcome

BC7: The mutation inventory covers implicit live reconciliation reached through reads/status and catchup, with no unfenced accepted ref transition.

## Check

## Plan

- [ ] Inventory actual side effects across dispatch and helpers, including moved-HEAD reconciliation; enumerate every coordinated effect in executable catalog coverage.
- [ ] Route authorized reconciliation through the same lock/fence/outcome owner or refuse it; accepted-state observation cannot silently publish.
- [ ] Test moved HEAD, catchup, wrong generations, read-triggered reconciliation and deletion tombstones, preserving unrelated scopes.
