---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B10 Verified replica read view"
paths:
  - "src/replica.rs"
  - "src/snapshot.rs"
  - "src/store.rs"
  - "tests/replica_snapshots.rs"
  - "tests/verification_snapshot.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test replica_snapshots --test verification_snapshot"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B11 Replica snapshot isolation"
---
# B11 Replica snapshot isolation

## Outcome

BC6: Exact accepted-revision snapshots have local ownership/capabilities and remain immutable while replication advances; unsupported source-tree transformations refuse explicitly.

## Check

## Plan

- [ ] Extend the snapshot owner to export the selected verified replica revision with exact source/tree and local ownership evidence.
- [ ] Preserve or explicitly reject raw bytes, dotfiles, modes, deletion, case/Unicode aliases, reserved names, symlink escape and unsupported filesystem transformations.
- [ ] Test incoming revision advancement during a build, wrong-owner cleanup, foreign output retention and native platform tree cases.
