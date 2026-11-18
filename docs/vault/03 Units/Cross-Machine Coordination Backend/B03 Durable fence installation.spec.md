---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B02 Authenticated coordinated sessions"
paths:
  - "src/store.rs"
  - "src/coordination.rs"
  - "src/lib.rs"
  - "tests/coordination_fencing.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test coordination_fencing"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B03 Durable fence installation"
---
# B03 Durable fence installation

## Outcome

BC2: Expected-generation fence installation, advance and revoke persist under the repository mutation lock and return an exact idempotent receipt before takeover acknowledgement.

## Check

## Plan

- [ ] Add bounded durable fence records for authority epoch, claim generation, actor and allowed scope without introducing a second lease clock.
- [ ] Implement install/advance/revoke using the existing repository lock and expected-current checks; missing/corrupt or stale state refuses safely.
- [ ] Test competing installers, changed-intent replay and crash/restart after persistence; unresolved takeover remains pending.
