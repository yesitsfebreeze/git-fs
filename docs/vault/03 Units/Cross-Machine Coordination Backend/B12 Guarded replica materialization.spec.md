---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 25
depends:
  - "B11 Replica snapshot isolation"
paths:
  - "src/replica.rs"
  - "src/store.rs"
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
  - "B12 Guarded replica materialization"
---
# B12 Guarded replica materialization

## Outcome

BC6: Optional explicit materialization uses scoped local replica authority and existing overwrite guards, reporting actual partial/applied state without exporting authority metadata.

## Check

## Plan

- [ ] Reuse guarded write/delete behavior for accepted replica bytes without requiring an exclusive source claim or altering local session ownership.
- [ ] Preserve credentials, read caches, locks, journals, indexes and capabilities; report conflicting/partial materialization rather than claiming synchronized disk.
- [ ] Test dirty and recreated files, native edit races, symlink/case collisions and retry after partial failure with before/after bytes.
