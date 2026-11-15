---
title: Reliable Concurrent Git-FS Contracts
status: specced
priority: 90
parent:
needs: []
footprint: [src/mcp.rs, src/overlay.rs, src/lib.rs, src/main.rs, src/runtime_identity.rs, manifest.md, tests/runtime_identity.rs, docs/reference.md, src/session_pin.rs, src/codex.rs, src/service.rs, tests/unit/session_pin.rs, tests/session_routing.rs, docs/session-routing.md, src/store.rs, src/snapshot.rs, tests/verification_snapshot.rs, docs/verification.md, src/review.rs, src/integrate.rs, src/verification.rs, tests/verification_receipt.rs, src/push.rs, tests/unit/integrate/tests.rs, tests/unit/store/tests.rs, tests/integration_recovery.rs, tests/guarded_push.rs, tests/concurrent_workflow.rs, tests/support/concurrent_workflow, tests/review_gate.rs, tests/tool_items.rs, src/limits.rs, justfile, docs/integration-handoffs.md]
checks: [cargo fmt --all -- --check, cargo clippy --all-targets --locked -- -D warnings, cargo test --locked, cargo build --release --locked]
created: 2026-11-14
updated: 2026-11-14
---

# Reliable Concurrent Git-FS Contracts

## Summary

Make Git-FS's runtime target, session binding, verification candidate and integration recovery explicit and testable for concurrent agents. Preserve the existing Git-backed overlay, freshness guards, review checks and guarded push implementation. This PRD covers Git-FS contracts plus coordinator/hub conformance fixtures and adoption handoffs; production Overwatch or Tailscale changes are separately owned work.

## Acceptance

- [ ] Runtime diagnostics identify the actual executable, supported MCP revisions, canonical target/session and schema fingerprint without modifying the workspace.
- [ ] Unsupported MCP protocol revisions are never silently advertised as supported; malformed input follows the documented contract.
- [ ] A coordinated caller cannot read under one identity and mutate another after routing/pin/CURRENT changes; trusted host routing cannot be overwritten by model input.
- [ ] Verification snapshots contain the exact merged integration candidate, preserve supported bytes/modes/deletions, isolate source inputs from other workers and refuse unsupported filesystem transformations.
- [ ] Versioned verification receipts bind the candidate, check policy, toolchain/platform and declared input scope; failed, cancelled, unknown, malformed or stale evidence cannot authorize integration.
- [ ] Integration revalidates required evidence against the final candidate, including when trunk advanced; existing review and secret checks remain enforced.
- [ ] Commit, disk/index synchronization and uncertain publication outcomes remain distinct across restart/reconcile; guarded recovery cannot duplicate a commit or overwrite foreign edits/staging.
- [ ] Stable push attempt identity and authoritative reconciliation survive lost acknowledgements, replay and independent remote updates; unresolved attempts are retained.
- [ ] Deterministic native-process concurrency/interruption scenarios pass on Windows and Unix for the same candidate source tree or verified content-equivalent trees, with exact commits, check-policy digests, platform exclusions and unavailable evidence recorded; relevant changes invalidate affected evidence.
- [ ] Overwatch execution/receipt-consumption and hub catalog-diagnosis contracts have fixtures and separate adoption handoffs; this PRD never claims those production deployments are complete.
- [ ] The reference documents source-isolation limits, compatibility behavior and recovery actions; performance claims are supported by a recorded baseline.

## Specs

- [[GFS-01 Runtime Identity and Protocol Negotiation]]
- [[GFS-02 Stable Session and Workspace Binding]]
- [[GFS-03 Exact Integration Candidate Snapshots]]
- [[GFS-04 Verification Receipts Bound to the Candidate]]
- [[GFS-05 Honest Integration Outcomes and Recovery]]
- [[GFS-06 Concurrent Native Acceptance and Adoption Handoffs]]

## Review

<!-- prd_review appends one line per round: - round 1 · 92/100 · digest … · date · reviewer.
     The digest covers this PRD's acceptance text and checks plus every linked unit's Plan items
     and checks, so editing any of them makes the round stale. 90/100 within 5 rounds. -->
- round 1 · 94/100 · digest f7b90ae4dc5cf241 · 2026-11-14 · /root/protocol_research
  - finding: No blocking findings. Same-candidate cross-platform evidence and measured-workload baseline refinements confirmed. External adoption and unavailable Unix execution remain explicit future work.

## Status

Nothing yet.

## Collection

<!-- prd_collect writes the receipt here: commit, review round, units, checks. -->

## Ownership boundaries

- Git-FS owns candidate content, identity, safe mutations and publication/recovery contracts.
- Overwatch owns plan/claims, check selection and execution, result storage and acceptance orchestration.
- Tailscale/hub owns authorized publication routing, generations, catalog comparison and client refresh diagnostics.

## External adoption handoffs

### Overwatch candidate verification consumer

Owner: Overwatch maintainer; resolve current canonical checkout and live claim before implementation.

Prerequisites: GFS-03 Exact Integration Candidate Snapshots, GFS-04 Verification Receipts Bound to the Candidate.

Execute configured checks in run-owned candidate workspaces with separate mutable outputs; emit bound receipts; require matching evidence for acceptance; preserve cancellation/restart state. Test against the real Git-FS API on exact commits. The currently inspected installed Overwatch checkout is not assumed to be the canonical development checkout.

### Tailscale publication and catalog diagnosis

Owner: Overwatch/Tailscale hub maintainer; resolve authorized Git-FS publication and live claim before implementation.

Prerequisites: GFS-01 Runtime Identity and Protocol Negotiation, GFS-02 Stable Session and Workspace Binding.

Compare same-authorized-publication direct backend, running backend, hub and client tool/schema identities; identify the first mismatch, preserve session identity, use only supported notifications/refresh mechanisms and distinguish unavailable service from stale client discovery. Do not infer failure from another project's tool catalog.


## Planning evidence and limits

Baseline: a39422c3cbf67487016f71e9eb414eef80bafc91 on feb at D:/dev/git-fs.

- No dedicated Git-FS publication is exposed in this chat; only language-1c1d is available.
- A Unix execution target has not been discovered; native Unix acceptance remains a future gate.
- The entire .overwatch directory is currently ignored by Git; the plan is saved in the local project ledger, not committed or synchronized. Preserve the existing ignore decision; a future publishing task must explicitly choose how vault notes travel.
- No full test suite is rerun for this planning task; prior a39422c evidence is not evidence for future implementation.
- Paper findings guide design; their guarantees are not automatically inherited by Git-FS.

Research: [[Git-FS Improvement Research 2026-11-14]]. The implementation acceptance boxes remain open.
