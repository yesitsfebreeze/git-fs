---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B08 Accepted-state query and hints"
paths:
  - "src/replica.rs"
  - "src/lib.rs"
  - "src/service.rs"
  - "manifest.md"
  - "tests/replica_snapshots.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test replica_snapshots --test coordination_contract"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B09 Replica object admission"
---
# B09 Replica object admission

## Outcome

BC5: A registered replica accepts only authorized immutable Git objects with a complete verified closure for an advertised authority revision, without redirecting into other refs or filesystem targets.

## Check

## Plan

- [ ] Use configured authenticated Git transport and existing object verification; request data cannot choose arbitrary remote URLs, commands or unrestricted objects.
- [ ] Validate project/authority/epoch, object format, authorized reachability and complete closure before replica publication; keep transfer outside the mutation critical section.
- [ ] Exercise wrong bindings, unauthorized reachable/unreachable objects, malformed refs, missing/corrupt closure and stale expected-old values; prove main/live/session and peer refs unchanged on refusal.
