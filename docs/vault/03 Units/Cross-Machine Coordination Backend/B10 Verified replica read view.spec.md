---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B09 Replica object admission"
paths:
  - "src/replica.rs"
  - "src/store.rs"
  - "src/files.rs"
  - "src/search.rs"
  - "src/service.rs"
  - "tests/replica_snapshots.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test replica_snapshots"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B10 Verified replica read view"
---
# B10 Verified replica read view

## Outcome

BC5/BC6: Replica refs advance through expected-old checks, and explicit replica read/search mode resolves the verified revision without dirty-disk or unrelated-live fallback.

## Check

## Plan

- [ ] Implement the allowlisted replica ref and observed/applied revision state through the existing store owner under scoped local replica authority.
- [ ] Extend read/glob/grep source selection for replica mode while retaining separately identified native inspection; read-only consumers need no source-writing claim.
- [ ] Test reordered states, authorized reset/rollback, additions/deletions, search agreement and concurrent local sessions without altering foreign refs.
