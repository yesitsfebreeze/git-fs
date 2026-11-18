---
kind: spec
prd: Cross-Machine Coordination Backend
status: target
effort: 1
agent_minutes: 30
depends:
  - "B01 Versioned coordination contract"
paths:
  - "src/mcp.rs"
  - "src/service.rs"
  - "src/runtime_identity.rs"
  - "tests/session_routing.rs"
  - "tests/coordination_contract.rs"
checks:
  - "cargo fmt --all -- --check"
  - "cargo test --locked --test session_routing --test coordination_contract"
owner:
since:
proof:
tags:
  - "unit/target"
  - "prd/cross-machine-coordination-backend"
aliases:
  - "B02 Authenticated coordinated sessions"
---
# B02 Authenticated coordinated sessions

## Outcome

BC1: Trusted host authentication binds each coordinated caller to one repository/project/session; request input cannot switch identity, borrow state or fall back to unfenced mutation.

## Check

## Plan

- [ ] Extend immutable MCP binding with trusted coordination context; reject aliases, foreign roots, supplied identities and borrowed sessions before backend access.
- [ ] Gate coordinated mutations on advertised support and preserve the separate legacy endpoint contract.
- [ ] Exercise real stdio identity/reconnect negatives and prove a replica read cannot establish authoritative session freshness; remote edits require a re-read on the bound authority connection.
