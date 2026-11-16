---
title: GFS-08 Candidate Snapshot MCP Contract Exposure
category: protocol
status: implemented
proposals: []
issues: []
updated: 2026-11-16
tags: [spec]
depends: [GFS-03 Exact Integration Candidate Snapshots]
checks: [cargo fmt --all -- --check, cargo test --locked --test candidate_tool_contract, cargo test --locked, cargo clippy --all-targets --locked -- -D warnings, cargo build --release --locked]
paths: [manifest.md, src/tool_result.rs, tests/candidate_tool_contract.rs]
proof: "015f4830e5b2f5987610dd6b88329bb40fbde3a3 guards: `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo test --locked --test candidate_tool_contract` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: none"
evidence: e01d99b00c7f811a
---

# GFS-08 Candidate Snapshot MCP Contract Exposure

## Summary

The native Git-FS candidate snapshot engine and GFS-03 verification are unreachable through the advertised MCP tool: the manifest snapshot schema and predispatch tool_result whitelist allow only id even though overlay::handle supports candidate kind, expected_tree, expected_review and release capability. Make tools/list, input validation, and execution agree, preserving closed-world validation and per-session release authority, then prove the real MCP path end to end.

## Rules

-

## Plan


- [x] Reproduce native MCP tools/list missing candidate snapshot fields and tools/call rejecting a valid candidate input before dispatch.
- [x] Extend the canonical manifest snapshot input schema for live and candidate kind, expected_tree, expected_review, id, capability with exact types and additionalProperties:false.
- [x] Align tool_result predispatch field whitelist and strict validation with the advertised manifest and overlay handler, preserving legacy live snapshot default and release rules.
- [x] Add a native MCP regression proving advertised schema, candidate creation with checked tree/review, own capability release, and exact Git tree/blobs.
- [x] Prove stale tree/review and wrong-session/invalid capability plus unsupported/unknown snapshot fields fail closed without ref or worktree mutation.
- [x] Run fmt, focused tests, complete native suite, strict all-target Clippy and locked release; normal-hook scoped publication and independently verified GitHub main SHA, then formal Overwatch evidence.

## Status

Native gitfs_snapshot advertises and accepts live/candidate snapshot fields including checked preview tree/review and owner-capability release. Two concurrent real MCP clients verify exact candidate tree/blob, fail-closed ownership/capability/stale/type/unknown checks, and clean release; normal-hook published GitHub main 015f4830e5b2f5987610dd6b88329bb40fbde3a3. Full locked suite, fmt, strict Clippy, release and five canonical guards passed.

## Open questions

-

## Related

- [[Home]]

## Sources

-
