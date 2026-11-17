---
kind: spec
prd: Git-fs Earlier Work
title: GFS-10 Opt-in Version-Pinned Fresh MCP Client
status: migrated
depends:
  - "Git-fs Earlier Work/GFS-07 Safe Versioned Runtime Staging and Adoption"
paths: []
checks: []
proof: "0257e05f7e2208b6ab34944f72f00a9c5faa2582 guards: `powershell.exe -NoProfile -File scripts/opt-in-client.ps1 -SelfTest` exit 0 (guard run on this same tree); limitations: No interactive Claude coding session launched. Claude mcp list/get subcommands enumerate persisted servers, not session-only --mcp-config (mcp get git-fs-pinned returned absent), so new UI session tool menu remains unverified. Strict MCP flag does not disable old installed plugin hooks. No authenticated Tailscale OAuth principal/session trust, reconnect, global plugin activation, or hot-reload; versioned artifact SHA attested at config creation and native smoke, not every later client start."
migrated_from: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# GFS-10 Opt-in Version-Pinned Fresh MCP Client

## Outcome

Allow a fresh, local, explicitly opted-in client to bind one exact staged Git-FS binary and unique coordinated session through a disposable MCP configuration, without touching existing installed plugin hooks, stable executable, running processes, or authenticated gateway; defer full plugin/hub adoption until separately owned gates.

## Check

Migrated: done in the old repository. Its proof is the old record in `proof`, quoted; nothing in this repository checked it. `unit_reopen` sets it back to target to do it again here.

## Plan

- [x] Verify current Git source, staged artifact SHA and installed plugin/hook/process invariants; inspect Overwatch claims
- [x] Implement a narrow opt-in per-client MCP configuration generator with path/hash/session/workspace validation and no global registration
- [x] Exercise disposable config self-test, no-overwrite behavior and malformed identity/hash rejections
- [x] Verify one fresh pinned native MCP process initializes, exposes exact retry_sync schema and reports intended session/workspace without touching old clients
- [x] Document CLI opt-in and the explicit non-support of installed hooks and authenticated Tailscale publication
- [x] Run focused PowerShell/native smoke, commit/push only owned files, verify remote SHA and record Overwatch evidence

## Before migration

The old values, for reference only; nothing here reads them.

- note: `01 Specs/GFS-10 Opt-in Version-Pinned Fresh MCP Client.md` in `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault`
- title: GFS-10 Opt-in Version-Pinned Fresh MCP Client
- category: build
- status: implemented
- proposals: none
- issues: none
- updated: 2026-11-16
- tags: `spec`
- depends: `GFS-07 Safe Versioned Runtime Staging and Adoption`
- checks: `powershell.exe -NoProfile -File scripts/opt-in-client.ps1 -SelfTest`
- paths: `scripts/opt-in-client.ps1`, `docs/opt-in-client.md`
- proof: 0257e05f7e2208b6ab34944f72f00a9c5faa2582 guards: `powershell.exe -NoProfile -File scripts/opt-in-client.ps1 -SelfTest` exit 0 (guard run on this same tree); limitations: No interactive Claude coding session launched. Claude mcp list/get subcommands enumerate persisted servers, not session-only --mcp-config (mcp get git-fs-pinned returned absent), so new UI session tool menu remains unverified. Strict MCP flag does not disable old installed plugin hooks. No authenticated Tailscale OAuth principal/session trust, reconnect, global plugin activation, or hot-reload; versioned artifact SHA attested at config creation and native smoke, not every later client start.
- evidence: fe16f4c337405b02

### Status

GFS-10: opt-in new local Claude MCP uses direct exact version-staged 8d83c2cd Git-FS executable (SHA256 1980E4D2...), new unique Git-ref-valid coordinated session, explicit workspace, immutable no-overwrite per-client JSON config; no installer/hooks/restarts. Native initialize/initialized/tools-list candidate snapshot and retry_sync plus exact gitfs_ls identity verified; SelfTest and guard upheld. Source 0257e05f7e2208b6ab34944f72f00a9c5faa2582 pushed to GitHub main and independently verified.
