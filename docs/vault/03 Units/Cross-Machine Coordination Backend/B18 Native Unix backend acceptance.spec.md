---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B17 Native Windows backend acceptance"
paths:
  - "docs/coordination.md"
checks:
  - "cargo test --locked --test coordination_contract --test coordination_fencing --test coordination_recovery --test replica_snapshots --test session_routing --test integration_recovery --test guarded_push"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B18 Native Unix backend acceptance"
---
# B18 Native Unix backend acceptance

## Outcome

BC8/BC10: The final identified native Unix backend passes the same finalized source or verified equivalent source as Windows, preserving native permission/symlink/process semantics, and both current platform receipts establish the versioned backend handoff.

## Check

## Plan

- [ ] Select the B17 qualified source and record exact executable/runtime/toolchain identities. A documentation-only receipt commit may reuse native proof only with explicit source/tree-equivalence evidence for every runtime, fixture and build input; any change to those inputs requires both native suites to rerun.
- [ ] Run extension and existing session/publication suites on native Unix with permissions, symlinks, locks and process lifecycle schedules.
- [ ] Record actual Unix proof and exclusions, then record the final versioned backend handoff only after both native suites pass for the same or explicitly equivalent runtime/fixture/build inputs; preserve independent backend acceptance without requiring parent final acceptance, and keep parent production activation downstream.
