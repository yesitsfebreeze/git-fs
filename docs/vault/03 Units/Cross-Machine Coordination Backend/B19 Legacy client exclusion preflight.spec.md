---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B16 Cross-repository conformance handoff"
paths:
  - "src/coordination.rs"
  - "src/runtime_identity.rs"
  - "src/store.rs"
  - "tests/coordination_contract.rs"
  - "tests/coordination_fencing.rs"
  - "docs/coordination.md"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_contract --test coordination_fencing"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B19 Legacy client exclusion preflight"
---
# B19 Legacy client exclusion preflight

## Outcome

BC1/BC9: Coordinated enable requires proved incompatible-store refusal or an exclusively isolated authority store with managed access; capability negotiation alone cannot leave old local writers sharing that store.

## Check

## Plan

- [ ] Identify the actual authority repository and serving clients; choose verified older-binary refusal or an isolated authoritative repository exposed only through managed access.
- [ ] Add activation evidence/capability checks that refuse unsafe mixed-client topology without deleting locks, killing peers or migrating drafts implicitly.
- [ ] Exercise a real older executable in an isolated fixture to prove the selected exclusion mechanism; retain detection/pause for administrative divergence without claiming to sandbox arbitrary Git.
