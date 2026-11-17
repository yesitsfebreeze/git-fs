---
kind: prd
status: open
priority: 100
needs: []
footprint: []
checks: []
created: 2026-11-15
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# Live Shared Agent Commits

## Goal

Make every successful Git-FS file mutation a durable, immediately peer-readable commit on a shared live revision, with optimistic conflict detection, isolated agent execution snapshots, and separate guarded main integration and remote synchronization.

## Acceptance

- [x] A successful edit or deletion returns a durable Git commit and becomes visible to a different agent's next read without catchup or materialization.
- [x] Concurrent nonoverlapping changes converge; conflicting edits are refused without overwriting either agent's bytes or history.
- [x] Agents can run builds and tests against isolated, exact source snapshots without mutating another agent's worktree or staging.
- [x] Interrupted/retried operations report exact published or uncertain commits, and existing review, secret scanning and guarded push remain intact.
- [x] The combined Windows project checks pass, Git-FS is integrated and pushed to origin/main, and the remote commit is verified.

## Status

Nothing yet.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `02 PRDs/Live Shared Agent Commits.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: Live Shared Agent Commits
- status: open
- priority: 100
- parent: 
- needs: none
- footprint: `src/store.rs`, `src/files.rs`, `src/overlay.rs`, `src/merge.rs`, `src/search.rs`, `src/service.rs`, `tests/`, `docs/reference.md`, `README.md`
- checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked`, `cargo build --release --locked`
- created: 2026-11-15
- updated: 2026-11-15

### Review

<!-- prd_review appends one line per round: - round 1 · 92/100 · digest … · date · reviewer.
     The digest covers this PRD's acceptance text and checks plus every linked unit's Plan items
     and checks, so editing any of them makes the round stale. 90/100 within 5 rounds. -->
- round 1 · 94/100 · digest ac21e91d5afe7625 · 2026-11-15 · /root/live_prd_review
  - finding: No blocking findings in the scoped local-live-commit contract. Reviewed current unit proof 317fe562adf0e86d at HEAD 7651c838 and tests for peer-visible edits, nonoverlap merge/conflict refusal, isolated snapshot cleanup, native capture, guarded push and staging/hooks preservation. Running gateway now exposes gitfs_snapshot; per-client backend isolation and cross-machine continuous replication remain separately planned limitations, not acceptance claims of this PRD.

### Collection

<!-- prd_collect writes the receipt here: commit, review round, units, checks. -->
