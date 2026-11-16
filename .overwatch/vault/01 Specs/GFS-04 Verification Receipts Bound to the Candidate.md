---
title: GFS-04 Verification Receipts Bound to the Candidate
category: protocol
status: implemented
proposals: []
issues: []
updated: 2026-11-15
tags: [spec]
depends: [GFS-02 Stable Session and Workspace Binding, GFS-03 Exact Integration Candidate Snapshots]
checks: [cargo fmt --all -- --check, cargo test --locked --test verification_receipt, cargo test --locked]
paths: [src/review.rs, src/integrate.rs, src/verification.rs, src/lib.rs, manifest.md, tests/verification_receipt.rs, docs/verification.md]
proof: "f37e85303a90937947e62bdc03584cb96c345476 guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test verification_receipt` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); limitations: Trust requires host-controlled receipt-directory ACLs and runner identity; JSON provenance is not cryptographic attestation and candidate checkout is not a sandbox. Native range checker does not receive destination remote ref; guarded push owns destination enforcement. Overwatch consumer/Tailscale gateway per-client deployment and cross-machine GFS05/GFS06 acceptance remain separate work."
evidence: db31edd5f4c6f5fa
---

# GFS-04 Verification Receipts Bound to the Candidate

## Summary

Define and enforce the Git-FS side of verification evidence so a passing result cannot authorize a different candidate. Overwatch owns execution and collection; this unit provides a versioned consumer contract and a narrow test coordinator fixture, not a second production check runner.

## Rules

-

## Plan


- [x] Define a versioned receipt schema with run/runner identity, repository and candidate binding, base/overlay identity, check-definition and policy digests, platform/toolchain/dependency identities, allowlisted environment scope, per-check status, input-integrity status, log digests, relevant output digests and explicit limitations.
- [x] Bind the receipt to the reviewed integration preview and configured check policy; validate it again against the final candidate under the existing publication validation path, including a newly advanced trunk.
- [x] Make unknown, cancelled, failed, malformed, missing-required-field, stale-policy and wrong-candidate evidence ineligible for a passing result; define any reuse policy explicitly.
- [x] Keep verification policy repository-configured and preserve existing mandatory review/secret/push checks. Accept evidence only through the configured trusted runner/store boundary; document that provenance records alone are not cryptographic proof or a security sandbox.
- [x] Add a small coordinator conformance fixture that runs a command in GFS-03's snapshot and emits evidence; verify log attribution, source mutation rejection and separate build outputs without creating a production orchestrator inside Git-FS.
- [x] Write the production Overwatch handoff with exact request/response examples and independent ownership/acceptance requirements; do not report the live Overwatch consumer or hub as upgraded by this Git-FS unit.

## Status

Versioned verification receipts now bind each reviewed candidate, runner/check policy, source and logs to one integration preview. Integration rechecks before commit; review-check and push-range validation reject forged, stale, failed, cancelled or changed evidence even without command-review policy. Ten native receipt tests plus full suite passed; source committed and pushed as f37e85303a90937947e62bdc03584cb96c345476.

## Open questions

-

## Related

- [[Home]]

## Sources

-

## Acceptance examples

- A passing receipt for candidate A is refused for B, even if the session name or command text is unchanged.
- A changed check definition/policy or a failed/cancelled run cannot reuse an earlier pass.
- The fixture proves candidate-to-command-to-log attribution, while the production Overwatch adapter remains an explicit separate handoff.

## Evidence and implementation limits

Current verification records are useful evidence but no generic exact-candidate verification handoff is exposed. Existing review revision and final-candidate checks are integration points, not replacement targets.

Use content-addressed identities and standard existing hash/JSON facilities. Do not add a signing service, new database, remote scheduler or SLSA-level claim. A missing required runner or receipt must be visible; no silent downgrade when policy requires verification.

## Research basis

Build-Systems, Vesta, SLSA. See [[Git-FS Improvement Research 2026-11-14]].
