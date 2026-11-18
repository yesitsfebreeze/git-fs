---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B20 Opt-in activation and rollback contract"
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
  - "B17 Native Windows backend acceptance"
---
# B17 Native Windows backend acceptance

## Outcome

BC8: After all B19 runtime and B20 fixture/lifecycle work, the identified final native Windows executable and source pass backend extension and existing session/publication fixtures with platform effects recorded.

## Check

## Plan

- [ ] Select the finalized B19/B20 candidate and record source, executable hash, schema, Git and OS identities; do not substitute package-version equality or require a future native acceptance receipt to complete B20.
- [ ] Run the focused native suites, including case/reserved paths, sharing/locks and owned-child outcomes; preserve actual artifacts and failure states.
- [ ] Record native Windows proof and exclusions in this unit; unavailable targets or timeouts stay unaccepted. Any later runtime, fixture or build-input change invalidates both platform receipts and requires both native suites to rerun; final backend handoff waits for B18.
