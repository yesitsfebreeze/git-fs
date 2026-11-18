---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B13 Fenced integration and undo"
paths:
  - "src/push.rs"
  - "src/coordination.rs"
  - "src/store.rs"
  - "tests/guarded_push.rs"
  - "tests/coordination_recovery.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test guarded_push --test coordination_recovery"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B14 Fenced push dispatch"
---
# B14 Fenced push dispatch

## Outcome

BC7: Guarded push validates installed delivery authority at admission and persists in-flight identity before dispatch, preserving hook/review checks and unknown outcomes.

## Check

## Plan

- [ ] Bind existing push operation/expected remote head and authorized handoff to the current delivery generation and destination scope.
- [ ] Persist admission/dispatch before launching the owned Git child; perform network waits outside the repository-wide lock and retain existing hook chaining.
- [ ] Test stale generation, changed remote head, foreign recipient, duplicate dispatch and lost response using the real remote ref.
