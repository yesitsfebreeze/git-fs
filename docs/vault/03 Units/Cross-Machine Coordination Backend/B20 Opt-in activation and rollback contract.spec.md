---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B19 Legacy client exclusion preflight"
paths:
  - "docs/coordination.md"
  - "docs/reference.md"
  - "tests/coordination_contract.rs"
  - "tests/coordination_recovery.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_contract --test coordination_recovery"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B20 Opt-in activation and rollback contract"
---
# B20 Opt-in activation and rollback contract

## Outcome

BC9/BC10: The capability-gated quiesce/reconcile/disable/re-enable lifecycle contract is prepared and rehearsed before final native acceptance, preserving stores, fences and unresolved outcomes without requiring a future B17/B18 acceptance receipt.

## Check

## Plan

- [ ] Document lifecycle and rollback steps with verified legacy-client exclusion, exact source/binary prerequisites and preserved receipts/refs/snapshots.
- [ ] Rehearse store-safe disable/re-enable and rollback against isolated fixtures, preserving the frozen Mosaic reference and refusing ambiguous unsettled effects.
- [ ] Prepare the final native acceptance and versioned handoff requirements for B17/B18; do not claim native or final backend acceptance here. Backend proof remains independent of parent final acceptance, and joint production activation remains downstream of accepted backend plus Overwatch route/lease/replica evidence.
