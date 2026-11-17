---
kind: spec
prd: Reliable Concurrent Git-FS Contracts
title: GFS-01 Runtime Identity and Protocol Negotiation
status: migrated
depends: []
paths: []
checks: []
proof: "b3f77959bedc073d5a5646879ef45b44b2f583fd guards: `cargo fmt --all -- --check` exit 0; `cargo test --locked --test runtime_identity` exit 0; `cargo test --locked` exit 0; limitations: Routed Codex gitfs_ls currently labels its inspection fingerprint as the standard tool schema profile; GFS-02 will propagate trusted routed profile through Context. The GFS-01 initialize identity's codex unbound state is explicit. No other GFS-01 acceptance gaps observed."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-01 Runtime Identity and Protocol Negotiation

## Outcome

Make the running Git-FS target and supported protocol observable without modifying a workspace. Reuse existing context, repository identity and tool descriptions. Fix the reproduced arbitrary protocol-version echo before considering a newer MCP migration.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Define one versioned runtime identity contract containing canonical workspace/store identity, repository mode, resolved session and resolution source, HEAD and overlay identity, package/build identity with explicit unknown fields, supported protocol revisions and a deterministic tool-schema fingerprint.
- [x] Return a requested MCP revision only when supported; otherwise return a genuinely supported revision, and define malformed or missing initialize-input behavior according to the supported specification.
- [x] Expose the identity through existing inspection and CLI/MCP entry points without creating a standalone store, changing refs, reading credentials or inferring executable version from checkout HEAD.
- [x] Compute the fingerprint from canonically ordered tool names and schemas so same-count schema changes are detectable; keep authorization-filtered publication comparisons in the hub.
- [x] Add compiled-process tests for supported and unknown revisions, read-only diagnostics, nested/standalone roots, Windows path identity and a checkout that differs from the executable build.

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-01 Runtime Identity and Protocol Negotiation.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-01 Runtime Identity and Protocol Negotiation
- category: protocol
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-15
- tags: `spec`
- checks: `cargo fmt --all -- --check`, `cargo test --locked --test runtime_identity`, `cargo test --locked`
- paths: `src/mcp.rs`, `src/overlay.rs`, `src/lib.rs`, `src/main.rs`, `src/runtime_identity.rs`, `manifest.md`, `tests/runtime_identity.rs`, `docs/reference.md`
- proof: b3f77959bedc073d5a5646879ef45b44b2f583fd guards: `cargo fmt --all -- --check` exit 0; `cargo test --locked --test runtime_identity` exit 0; `cargo test --locked` exit 0; limitations: Routed Codex gitfs_ls currently labels its inspection fingerprint as the standard tool schema profile; GFS-02 will propagate trusted routed profile through Context. The GFS-01 initialize identity's codex unbound state is explicit. No other GFS-01 acceptance gaps observed.
- evidence: 1c62a980e9b8a009

### Status


GFS-01 runtime identity and protocol negotiation implemented in bfdf9f6f85dad977466ce1f69ff7b94060dfc2b8. The public, read-only identity exposes root/store/repository/session/build/protocol/schema details without creating a store; MCP initialize refuses malformed inputs and negotiates only implemented revisions. Verified all three canonical guards on clean combined main b3f77959bedc073d5a5646879ef45b44b2f583fd (which additionally includes independently owned e448296 reconcile fix and b3f77959 formatting).

### Acceptance examples


- Requesting protocol revision 2099-01-01 never returns that revision as supported.
- Equivalent schemas have the same fingerprint; a schema-only change changes it even when the tool count stays equal.
- An identity read leaves files, refs and staging unchanged, including in a plain directory.

### Evidence and implementation limits


Source src/mcp.rs echoes the request's protocolVersion; the installed executable was probed during planning. Only the language-1c1d catalog is exposed here; installed/direct and client catalogs differ, but the responsible boundary is not yet established.

New files in paths are proposed implementation/test locations. Do not claim latest MCP support or successful client refresh. Use an explicit supported-version set and preserve the currently supported host contracts.

### Research basis


MCP-Lifecycle, MCP-Tools. See [[Git-FS Improvement Research 2026-11-14]].
