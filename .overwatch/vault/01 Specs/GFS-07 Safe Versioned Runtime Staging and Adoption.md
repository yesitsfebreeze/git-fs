---
title: GFS-07 Safe Versioned Runtime Staging and Adoption
category: build
status: implemented
proposals: []
issues: []
updated: 2026-11-16
tags: [spec]
depends: [GFS-05 Honest Integration Outcomes and Recovery]
checks: [powershell.exe -NoProfile -File scripts/stage-runtime.ps1 -SelfTest, cargo fmt --all -- --check, cargo clippy --all-targets --locked -- -D warnings, cargo test --locked, cargo build --release --locked]
paths: [scripts/stage-runtime.ps1, docs/runtime-rollout.md]
proof: "56b7b522088ae6d5704d72ec87caadaecaa0e692 guards: `powershell.exe -NoProfile -File scripts/stage-runtime.ps1 -SelfTest` exit 0 (guard run on this same tree); `cargo fmt --all -- --check` exit 0 (guard run on this same tree); `cargo clippy --all-targets --locked -- -D warnings` exit 0 (guard run on this same tree); `cargo test --locked` exit 0 (guard run on this same tree); `cargo build --release --locked` exit 0 (guard run on this same tree); limitations: No plugin manifest or hooks re-pointed, no service or active MCP processes restarted. Multi-client authenticated gateway identity, plugin orphan-branch/config adoption, and GFS06 true two-client acceptance are separate guarded follow-ups; staged executable alone does not activate retry_sync for existing clients."
evidence: ab57e60f99fae760
---

# GFS-07 Safe Versioned Runtime Staging and Adoption

## Summary

Build and stage git-fs native executables side by side under immutable Git-commit version paths, proving binary hashes and a fresh MCP/CLI contract without replacing a running executable or changing installed plugin hooks. Document the separate coordinated new-client launcher, plugin orphan-branch and authenticated gateway rollout gates; do not perform in-use process restarts or client activation in this unit.

## Rules

-

## Plan


- [x] Audit installed binary and source versions, active process paths and hardcoded plugin invocation paths; preserve installed clients
- [x] Add an idempotent non-overwriting versioned staging script with source-tree and payload freshness guards
- [x] Exercise clean staging, same-SHA replay and mismatched-target refusal in a disposable versioned test root
- [x] Stage exact release executable beside installed plugin without replacing active binary or hooks; verify hashes and fresh CLI identity
- [x] Document new-client pinning, hot handoff, installed catalog verification and held gateway-client authentication acceptance
- [x] Pass platform-appropriate formatting, strict Clippy, full native tests, release build and staged artifact checks; publish exact Git SHA and evidence

## Status

GFS-07: immutable side-by-side versioned Windows git-fs staging only; pinned source and clean-input pre/post-build checks; SHA256 collision refusal and stable plugin preservation; live disposable MCP JSON-RPC revision/schema smoke; full five guards upheld; git source 56b7b522088ae6d5704d72ec87caadaecaa0e692 pushed and verified against GitHub main.

## Open questions

-

## Related

- [[Home]]

## Sources

-
