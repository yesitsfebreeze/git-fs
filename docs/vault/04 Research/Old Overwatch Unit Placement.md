---
kind: research
created: 2026-11-17
source: "D:\\dev\\git-fs\\.overwatch\\worktrees\\migrate\\.overwatch\\vault"
---
# Old Overwatch Unit Placement

Where each spec of the old overwatch vault `D:\dev\git-fs\.overwatch\worktrees\migrate\.overwatch\vault` goes in this vault: step 8, [[08 Switch Overwatch Over.spec|08 Switch Overwatch Over]]. `vault_migrate` (or `overwatch migrate`) writes this list on a dry run and keeps every decision already in it; the real run refuses while a problem below is open.

Each old spec has a `prd` (a real PRD: an old one or a new one named for its outcome; never Unassigned), a `name` (its note name there), and an `outcome`: `migrated` (done work, closed with its old proof), `superseded` (not copied; `covered by` names the unit here as <prd>/<spec>) or `open` (kept as target work, with `paths` and `checks` of this repository, each in backticks). A spec of the flat `01 Specs` that is a copy of a `prds/` spec follows that spec. A PRD is `kept` (written with its `status`) or `superseded`.

## Summary

- old specs: 11 (0 of them copies in the flat layout)
- migrated: 11 · superseded: 0 · open: 0 · unplaced: 0
- notes to write: 3 PRD(s), 1 of them new, and 11 spec(s); PRDs superseded: 0

## Problems

- none

## PRDs

### Live Shared Agent Commits
- from: `02 PRDs/Live Shared Agent Commits.md`
- outcome: kept
- status: open
- priority: 100
- needs: 
- covered by: 
- footprint: 
- checks: 
- why: 

### Reliable Concurrent Git-FS Contracts
- from: `02 PRDs/Reliable Concurrent Git-FS Contracts.md`
- outcome: kept
- status: specced
- priority: 90
- needs: 
- covered by: 
- footprint: 
- checks: 
- why: 

### Git-fs Earlier Work
- from: new
- outcome: kept
- status: open
- priority: 10
- needs: 
- covered by: 
- goal: The work the old overwatch plan of this project held without a PRD, kept so it can be finished or closed here.
- acceptance: 
- footprint: 
- checks: 
- why: The old units that had no PRD, placed here by the owner's default rule (08 Decisions/How the Last Pass Places Old Units).

## Units

### `01 Specs/GFS Live Shared Commit Acceptance.md`
- title: GFS Live Shared Commit Acceptance
- old status: implemented
- prd: Live Shared Agent Commits
- name: GFS Live Shared Commit Acceptance
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-01 Runtime Identity and Protocol Negotiation.md`
- title: GFS-01 Runtime Identity and Protocol Negotiation
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-01 Runtime Identity and Protocol Negotiation
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-02 Stable Session and Workspace Binding.md`
- title: GFS-02 Stable Session and Workspace Binding
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-02 Stable Session and Workspace Binding
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-03 Exact Integration Candidate Snapshots.md`
- title: GFS-03 Exact Integration Candidate Snapshots
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-03 Exact Integration Candidate Snapshots
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-04 Verification Receipts Bound to the Candidate.md`
- title: GFS-04 Verification Receipts Bound to the Candidate
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-04 Verification Receipts Bound to the Candidate
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-05 Honest Integration Outcomes and Recovery.md`
- title: GFS-05 Honest Integration Outcomes and Recovery
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-05 Honest Integration Outcomes and Recovery
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-06 Concurrent Native Acceptance and Adoption Handoffs.md`
- title: GFS-06 Concurrent Native Acceptance and Adoption Handoffs
- old status: implemented
- prd: Reliable Concurrent Git-FS Contracts
- name: GFS-06 Concurrent Native Acceptance and Adoption Handoffs
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-07 Safe Versioned Runtime Staging and Adoption.md`
- title: GFS-07 Safe Versioned Runtime Staging and Adoption
- old status: implemented
- prd: Git-fs Earlier Work
- name: GFS-07 Safe Versioned Runtime Staging and Adoption
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-08 Candidate Snapshot MCP Contract Exposure.md`
- title: GFS-08 Candidate Snapshot MCP Contract Exposure
- old status: implemented
- prd: Git-fs Earlier Work
- name: GFS-08 Candidate Snapshot MCP Contract Exposure
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-09 Candidate Snapshot Commit-Tree Stdin Closure.md`
- title: GFS-09 Candidate Snapshot Commit-Tree Stdin Closure
- old status: implemented
- prd: Git-fs Earlier Work
- name: GFS-09 Candidate Snapshot Commit-Tree Stdin Closure
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why: 

### `01 Specs/GFS-10 Opt-in Version-Pinned Fresh MCP Client.md`
- title: GFS-10 Opt-in Version-Pinned Fresh MCP Client
- old status: implemented
- prd: Git-fs Earlier Work
- name: GFS-10 Opt-in Version-Pinned Fresh MCP Client
- outcome: migrated
- covered by: 
- paths: 
- checks: 
- why:
