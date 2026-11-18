---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B04 Durable mutation operation identity"
paths:
  - "src/files.rs"
  - "src/store.rs"
  - "src/coordination.rs"
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
  - "B05 Fenced file publication"
---
# B05 Fenced file publication

## Outcome

BC2/BC3: Coordinated write/edit/delete validate the installed fence inside the shared-publication lock and report preserved session-only commits separately from shared acceptance.

## Check

## Plan

- [ ] Thread trusted fence and operation context through existing file mutations without changing preimage or three-way merge semantics.
- [ ] Record exact session/live stage outcomes around the existing commit and compare-and-swap boundaries, permitting bounded recheck/merge/hash work inside the lock.
- [ ] Use deterministic barriers for revocation before/after acceptance, disjoint and overlapping edits, process death and lost replies; assert refs, bytes and retained attempts.
