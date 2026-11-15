---
title: GFS-04 Verification Receipts Bound to the Candidate
category: protocol
status: target
proposals: []
issues: []
updated: 2026-11-14
tags: [spec]
depends: [GFS-02 Stable Session and Workspace Binding, GFS-03 Exact Integration Candidate Snapshots]
checks: [cargo fmt --all -- --check, cargo test --locked --test verification_receipt, cargo test --locked]
paths: [src/review.rs, src/integrate.rs, src/verification.rs, src/lib.rs, manifest.md, tests/verification_receipt.rs, docs/verification.md]
---

# GFS-04 Verification Receipts Bound to the Candidate

## Summary

Define and enforce the Git-FS side of verification evidence so a passing result cannot authorize a different candidate. Overwatch owns execution and collection; this unit provides a versioned consumer contract and a narrow test coordinator fixture, not a second production check runner.

## Rules

-

## Plan

- [ ] Define a versioned receipt schema with run/runner identity, repository and candidate binding, base/overlay identity, check-definition and policy digests, platform/toolchain/dependency identities, allowlisted environment scope, per-check status, input-integrity status, log digests, relevant output digests and explicit limitations.
- [ ] Bind the receipt to the reviewed integration preview and configured check policy; validate it again against the final candidate under the existing publication validation path, including a newly advanced trunk.
- [ ] Make unknown, cancelled, failed, malformed, missing-required-field, stale-policy and wrong-candidate evidence ineligible for a passing result; define any reuse policy explicitly.
- [ ] Keep verification policy repository-configured and preserve existing mandatory review/secret/push checks. Accept evidence only through the configured trusted runner/store boundary; document that provenance records alone are not cryptographic proof or a security sandbox.
- [ ] Add a small coordinator conformance fixture that runs a command in GFS-03's snapshot and emits evidence; verify log attribution, source mutation rejection and separate build outputs without creating a production orchestrator inside Git-FS.
- [ ] Write the production Overwatch handoff with exact request/response examples and independent ownership/acceptance requirements; do not report the live Overwatch consumer or hub as upgraded by this Git-FS unit.

## Status

What exists today, with the ledger unit that owns the rest.

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
