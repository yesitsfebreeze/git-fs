---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends: []
paths:
  - "manifest.md"
  - "src/service.rs"
  - "src/runtime_identity.rs"
  - "tests/coordination_contract.rs"
  - "tests/fixtures/coordination/"
  - "docs/coordination.md"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_contract"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B01 Versioned coordination contract"
---
# B01 Versioned coordination contract

## Outcome

BC1/BC9: The opt-in extension has bounded capability, request and outcome schemas plus executable-neutral fixture inputs; legacy callers retain their protocol and the Native Mosaic Port oracle stays unchanged.

## Check

## Plan

- [ ] Define protocol version, bound identity, scope, expected revisions, operation id and stage-specific outcomes in the existing contract owner.
- [ ] Add valid/invalid golden inputs and capability tests, including unsupported versions and incomplete mutation coverage; fixtures identify the executable under test.
- [ ] Document the serving source/binary/schema baseline and run the focused contract checks before handoff.
