---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B15 Takeover with in-flight publication"
paths:
  - "tests/coordination_contract.rs"
  - "tests/coordination_fencing.rs"
  - "tests/coordination_recovery.rs"
  - "tests/replica_snapshots.rs"
  - "tests/fixtures/coordination/"
  - "docs/coordination.md"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_contract --test coordination_fencing --test coordination_recovery --test replica_snapshots"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B16 Cross-repository conformance handoff"
---
# B16 Cross-repository conformance handoff

## Outcome

BC8/BC10: The versioned executable-neutral backend fixtures and adoption contract identify exact serving builds and permit isolated candidate integration before final acceptance.

## Check

## Plan

- [ ] Package concrete inputs/assertions for identity, fencing, outcomes, hint/query recovery, replica view and release admission; preserve the frozen Mosaic oracle.
- [ ] Run two independent repositories/processes through the accepted-state lifecycle, checking actual refs/bytes/receipts and overlapping disjoint work intervals.
- [ ] Define evidence stages: candidate conformance permits isolated Overwatch integration; backend acceptance uses its own native fixtures and handoff; parent production activation follows accepted backend rather than a circular parent-completion dependency.
