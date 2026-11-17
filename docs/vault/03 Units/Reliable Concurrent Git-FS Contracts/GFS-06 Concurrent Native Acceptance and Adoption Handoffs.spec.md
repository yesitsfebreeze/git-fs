---
kind: spec
prd: Reliable Concurrent Git-FS Contracts
title: GFS-06 Concurrent Native Acceptance and Adoption Handoffs
status: migrated
depends:
  - "Reliable Concurrent Git-FS Contracts/GFS-05 Honest Integration Outcomes and Recovery"
paths: []
checks: []
proof: "339af068c85b64e6225492f19e0a24401a7708f1 guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo test --locked --test concurrent_workflow --test runtime_identity --test session_routing --test verification_snapshot --test verification_receipt --test integration_recovery --test guarded_push --test tool_items` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: Ubuntu rustfmt 1.11 fails cargo fmt --check on untouched src/mcp.rs and tests/unit/integrate/tests.rs (Windows rustfmt 1.10 passes); repository-wide Unix fmt needs a pinned formatter. Overwatch adapter, hub publication comparison, two-OAuth-principal and ChatGPT refresh acceptance are external, unclaimed and not done. GitHub issue #1 stays open. Commit 339af068 not pushed to origin."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-06 Concurrent Native Acceptance and Adoption Handoffs

## Outcome

Demonstrate the combined contracts through real Git-FS processes on Windows and Unix, and provide precise adoption contracts for Overwatch and the Tailscale hub. Test the Git-FS boundaries with fixtures; production deployment in other repositories remains separately owned work.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Add a bounded native-process acceptance harness with operation traces and test-only deterministic barriers around routing, preimage checks, snapshot creation, ref publication and disk/index synchronization; each failing schedule must be replayable without timing-only sleeps.
- [x] Run two MCP clients through read/edit/delete/capture/materialize/candidate verification/integration against temporary repositories, and independently inspect resulting refs, bytes, staging and remote state with Git.
- [x] Exercise same-line conflicts, non-overlapping changes, manual edits, permission errors, process interruption, receipt replay and malformed/unknown protocol versions while preserving all existing regression suites.
- [x] Run applicable tests on native Windows and an available Unix machine against the same candidate source tree, or independently verified content-equivalent trees; record exact commits/tree identities, check-policy digest, toolchain/platform, pass/fail and excluded cases. Relevant source or check-policy changes invalidate affected evidence. Unix-only review_gate coverage is not passed by a Windows zero-test result.
- [x] Create an Overwatch consumer handoff and a hub capability-diagnosis handoff with owners-to-resolve, required API versions, request/receipt fixtures and acceptance cases; map the canonical external checkout and live claims before anyone implements either.
- [x] Document same-authorized-publication comparison of installed/direct backend/hub/client schemas and generations, pagination and supported change notifications; report an unrefreshable catalog as stale discovery, without claiming notification receipt fixed ChatGPT.
- [x] Update the reference and local verification recipe with the exact limits, collect a small non-gating baseline of latency/tool calls/conflict retries with repository size, operation mix, concurrency, sample count and latency summary, and reconcile issue 1 acceptance evidence without closing or posting externally unless separately authorized.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-06 Concurrent Native Acceptance and Adoption Handoffs.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-06 Concurrent Native Acceptance and Adoption Handoffs
- category: acceptance
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-16
- tags: `spec`
- depends: `GFS-05 Honest Integration Outcomes and Recovery`
- checks: `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked --test concurrent_workflow --test runtime_identity --test session_routing --test verification_snapshot --test verification_receipt --test integration_recovery --test guarded_push --test tool_items`, `cargo test --locked`, `cargo build --release --locked`
- paths: `tests/concurrent_workflow.rs`, `tests/support/concurrent_workflow`, `tests/runtime_identity.rs`, `tests/session_routing.rs`, `tests/verification_snapshot.rs`, `tests/verification_receipt.rs`, `tests/integration_recovery.rs`, `tests/guarded_push.rs`, `tests/review_gate.rs`, `tests/tool_items.rs`, `src/limits.rs`, `src/store.rs`, `src/integrate.rs`, `justfile`, `docs/reference.md`, `docs/integration-handoffs.md`
- proof: 339af068c85b64e6225492f19e0a24401a7708f1 guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo test --locked --test concurrent_workflow --test runtime_identity --test session_routing --test verification_snapshot --test verification_receipt --test integration_recovery --test guarded_push --test tool_items` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: Ubuntu rustfmt 1.11 fails cargo fmt --check on untouched src/mcp.rs and tests/unit/integrate/tests.rs (Windows rustfmt 1.10 passes); repository-wide Unix fmt needs a pinned formatter. Overwatch adapter, hub publication comparison, two-OAuth-principal and ChatGPT refresh acceptance are external, unclaimed and not done. GitHub issue #1 stays open. Commit 339af068 not pushed to origin.
- evidence: d3441008736ceb8e

### Status


GFS-06: test-only deterministic schedule points (GITFS_TEST_BARRIER_DIR / GITFS_TEST_BARRIER_PAUSE; route, preimage, publish, snapshot, sync) in src/store.rs write a per-pid trace and pause on harness go-files. concurrent_workflow (7 native two-process tests) replays a same-line publication between preimage check and locked live re-check (refused, live conflict, never publishes) and a disjoint peer held behind locked publication (both land), with Git inspection of refs, blobs, HEAD and disk. Commit 339af068 on main; same source/policy blob list (digest d0166569...) passes clippy, targeted suites, full cargo test and release build on Windows 11 (rustc 1.99) and Ubuntu WSL2 (rustc 1.100, review_gate 1/1). Overwatch consumer and hub diagnosis handoffs, final evidence and non-gating baseline in docs/integration-handoffs.md; reference documents the schedule points.

### Acceptance examples


- The recorded schedule and operation trace reproduce a failing case, and independent Git inspection establishes the expected final state.
- Windows and Unix outcomes are recorded separately; missing machine access or skipped platform cases remain limitations and block cross-platform acceptance.
- Git-FS contract completion, external adapter implementation and live hub/client adoption are separate states; none is inferred from another.

### Evidence and implementation limits


Existing native tests guarded_push/tool_items, Unix-only review_gate, store/integration concurrency regressions and Windows child-process cancellation form the baseline. Reuse rather than replace them.

Use local execution and exact commit handoffs; hosted CI availability is not a prerequisite. No performance target is asserted until a baseline exists. Production Overwatch and hub changes must have separate repository-scoped claims and checks.

### Research basis


CHESS, Crash-Testing, RIFL, MCP-Tools, Git-Worktrees. See [[Git-FS Improvement Research 2026-11-14]].
