---
title: GFS-03 Exact Integration Candidate Snapshots
category: build
status: implemented
proposals: []
issues: []
updated: 2026-11-15
tags: [spec]
depends: [GFS-01 Runtime Identity and Protocol Negotiation]
checks: [cargo fmt --all -- --check, cargo test --locked --test verification_snapshot, cargo test --locked]
paths: [src/store.rs, src/snapshot.rs, src/lib.rs, src/overlay.rs, manifest.md, tests/verification_snapshot.rs, docs/verification.md]
proof: "07be9598ede1b017f547a610f6b30e54832962c2 guards: `cargo fmt --all -- --check` exit 0; `cargo test --locked --test verification_snapshot` exit 0; `cargo test --locked` exit 0; limitations: Not hermetic: external Git/compiler/dependencies and mutable build output remain outside snapshot integrity contract. Unsafe symlink, submodule, filter or line-ending transformations are rejected rather than exported."
evidence: 37b67c9c08f394f3
---

# GFS-03 Exact Integration Candidate Snapshots

## Summary

Expose a pinned, run-owned snapshot of the exact merged integration candidate. Git-FS owns candidate content and identity; Overwatch remains responsible for selecting and running checks. A shared checkout's before/after hashes cannot establish source isolation.

## Rules

-

## Plan


- [x] Derive the candidate from the same preview/reviewed_tree path used by integration, including current trunk and intended session changes; bind repository, session, base, overlay and final tree identities.
- [x] Create one run-owned isolated snapshot/export path using existing Git object and exporter facilities or standard Git worktree support; select one supported implementation and verify raw bytes, deletions, binary data and executable modes against the manifest.
- [x] Specify symlink, submodule, filter and Windows EOL behavior; refuse unsupported or transformed inputs explicitly instead of silently verifying a different tree.
- [x] Record an ownership manifest and bounded lifecycle; reject an existing/foreign destination, preserve user branches, shared files and staging, and allow cleanup only for the exact owned run.
- [x] Define the coordinator handoff and conformance fixture: checks use a run-owned working directory and mutable build-output directories; external tools, dependencies and environment scope are declared limitations rather than assumed hermetic.
- [x] Test concurrent shared-checkout changes, two snapshots, advanced trunk, deletion/binary/mode cases, cancellation and foreign-content cleanup refusal.

## Status

GFS-03 candidate snapshots export exact reviewed Git tree from shared trunk and session overlay into isolated detached worktrees with repository, session, source and tree provenance, unguessable run-specific cleanup capability, exact raw bytes/modes/deletions, and fail-closed unsafe transforms. Integrated and published main 07be9598ede1b017f547a610f6b30e54832962c2; fmt, focused candidate tests 6/6, complete locked suite, Clippy -D warnings, release build, and all 3 Overwatch guards passed.

## Open questions

-

## Related

- [[Home]]

## Sources

-

## Acceptance examples

- Snapshot tree identity is the integration candidate, not merely the session tip.
- Another worker can modify the shared checkout without changing the snapshot's input bytes or run ownership.
- No unrequested disk/staged/untracked content enters the candidate; unsupported filesystem entries are explicit failures.

## Evidence and implementation limits

The current read-only reviewer has a private snapshot exporter in scripts/codex-review.py, but there is no general validation snapshot handoff. Existing reviewed_tree and Git object access are reuse points.

A detached worktree isolates working files/HEAD/index but shares repository state; worktree lock only prevents pruning. Do not describe source isolation as a sandbox, hermetic execution, reproducible outputs or immutability against arbitrary privileged shell programs. Preserve Git metadata if chosen checks need it; the chosen mechanism and its limitations must be recorded.

## Research basis

Build-Systems, Vesta, Git-Worktrees. See [[Git-FS Improvement Research 2026-11-14]].
