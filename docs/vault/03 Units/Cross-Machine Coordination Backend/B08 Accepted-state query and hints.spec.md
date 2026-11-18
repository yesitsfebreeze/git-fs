---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B07 Reconciliation side-effect coverage"
paths:
  - "src/coordination.rs"
  - "src/service.rs"
  - "src/mcp.rs"
  - "manifest.md"
  - "tests/coordination_recovery.rs"
  - "tests/coordination_contract.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_recovery --test coordination_contract"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B08 Accepted-state query and hints"
---
# B08 Accepted-state query and hints

## Outcome

BC4: A bounded authoritative query reports one consistent accepted revision/generation/outcome; optional invalidation hints can be dropped without losing recoverability.

## Check

## Plan

- [ ] Expose accepted-state and observation tokens from durable refs/fences/receipts without unfenced mutation; identify incomplete observations explicitly.
- [ ] Emit bounded typed hints after accepted transitions, with no generic durable event broker or command replay.
- [ ] Test suppressed, duplicate and reordered hints, commit-before-notify crash, restart and changing state; queries recover the accepted result without requiring another edit.
