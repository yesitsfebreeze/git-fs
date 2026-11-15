# Git-FS Improvement Research - 2026-11-14

## Baseline

Machine: feb. Repository: D:/dev/git-fs. Branch: main. HEAD: a39422c3cbf67487016f71e9eb414eef80bafc91. Clean tracked tree at planning start. Installed Git-FS 4.4.0 lists 17 tools. Only language-1c1d is exposed in this ChatGPT tool catalog; it is used as an explicitly targeted shell transport, not as the Git-FS planning ledger.

The installed Git-FS initialize probe requested 2099-01-01 and received 2099-01-01, reproducing unsupported-version echo. The probe returned 17 tools; canonical sorted tool/schema SHA-256: f1c407207fd77d256161d9a1f1a5841dc14508970a0bfb30964c1886765bae59. No files or refs were changed by that probe.

## Method

Six SciSpace searches, data extraction from five selected papers, primary-source verification, official Git/MCP/provenance documentation, and independent review of current Rust implementation. The source/installed-executable guards previously passed at the baseline commit; existing logs record 146 passing Windows tests. These are not test results for the proposed implementation.

## Sources and transfer limits

### Build-Systems - Build Systems a la Carte (2018)

SciSpace: https://scispace.com/papers/build-systems-a-la-carte-26orcifjjf

Primary source: https://simon.peytonjones.org/assets/pdfs/build-systems-original.pdf

Supported principle: Verification validity depends on preserved inputs and outputs consistent with declared tasks/dependencies.

Transfer limit: Undeclared environment/network inputs and nondeterministic tasks need an explicit scope; hashes alone do not make a build hermetic.

### Vesta - The Vesta Approach to Software Configuration Management (2001)

SciSpace: https://scispace.com/papers/the-vesta-approach-to-software-configuration-management-22hwsqa0wl

Primary source: https://vesta.sourceforge.net/doc/pubs/src-rr-168/

Supported principle: Retained source/tool inputs and sufficiently complete build descriptions support repeatability.

Transfer limit: A Git checkout with uncontrolled dependencies cannot inherit Vesta's repeatability guarantees.

### CHESS - Finding and Reproducing Heisenbugs in Concurrent Programs (2008)

SciSpace: https://scispace.com/papers/finding-and-reproducing-heisenbugs-in-concurrent-programs-2zb765d6ea

Primary source: https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/chess-osdi2008-chess.pdf

Supported principle: Controlled schedules and repeatable starting state make concurrency failures reproducible.

Transfer limit: Git-FS needs application/process barriers; thread scheduling alone does not cover external Git or filesystem behavior.

### Crash-Testing - Finding Crash-Consistency Bugs with Bounded Black-Box Crash Testing (2018)

SciSpace: https://scispace.com/papers/finding-crash-consistency-bugs-with-bounded-black-box-crash-3q25dgn55g

Primary source: https://www.usenix.org/system/files/osdi18-mohan.pdf

Supported principle: Enumerate bounded operation sequences and interruption points, then assert explicit recovery invariants.

Transfer limit: Process termination is not the paper's filesystem power-loss model; bounds are explicit and not exhaustive beyond that scope.

### RIFL - Implementing Linearizability at Large Scale and Low Latency (2015)

SciSpace: https://scispace.com/papers/implementing-linearizability-at-large-scale-and-low-latency-2mm78x219m

Primary source: https://web.stanford.edu/~ouster/cgi-bin/papers/rifl.pdf

Supported principle: Stable request identity and durable completion records support retry safety when the required atomicity assumptions hold.

Transfer limit: A remote Git update and a local receipt are not atomic together; retain unknown outcomes and reconcile instead of claiming exactly once.

### MCP-Lifecycle - MCP 2025-06-18 Lifecycle

Primary source: https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle

Supported principle: Negotiate a protocol version the server actually supports and advertise truthful capabilities.

Transfer limit: Fixing this handshake does not establish newer protocol support.

### MCP-Tools - MCP 2025-06-18 Tools

Primary source: https://modelcontextprotocol.io/specification/2025-06-18/server/tools

Supported principle: Use tools/list with pagination and advertise listChanged only when the corresponding notification behavior is supported.

Transfer limit: Notifications do not prove that a particular client refreshed. Compare the same authorized publication/principal.

### Git-Worktrees - Git worktree documentation

Primary source: https://git-scm.com/docs/git-worktree

Supported principle: A detached worktree provides separate working files, HEAD and index.

Transfer limit: Objects/refs/default configuration are shared and worktree lock protects against pruning, not file writes.

### SLSA - SLSA v1.2 Build Provenance

Primary source: https://slsa.dev/spec/v1.2/build-provenance

Supported principle: Distinguish input definitions/dependencies, execution details and output artifact identity.

Transfer limit: Adopting receipt fields does not establish a SLSA level or trusted execution boundary.

## Observed facts, risks and proposals

Observed: protocol-version echo; per-call fallback session resolution; no general candidate verification handoff; replay/reconcile does not return the same worktree recovery detail as commit. Existing guarded push, exact review binding and atomic Git ref publication already exist.

Not reproduced: accidental cross-session writes, mixed-source test contamination, data loss on recovery, or the cause of catalog differences. Those are risks and test scenarios, not newly proven defects.

Proposal: test the exact final candidate in a run-owned workspace and bind evidence to its inputs and declared execution scope. Before/after hashing of a shared checkout cannot rule out a transient modification during testing. This is an engineering inference, not a guarantee transferred from a paper.

The 17-vs-16 observation compares an installed binary with this client catalog. Diagnose the same authorized publication/principal at every boundary before assigning a cause.

## Plan

PRD: [[Reliable Concurrent Git-FS Contracts]]. Six units, 35 Plan items, 11 acceptance checks. Wave 1: GFS-01. Wave 2: GFS-02 and GFS-03. Wave 3: GFS-04. Wave 4: GFS-05. Wave 5: GFS-06.

## Scope

Planning only. No implementation unit is claimed or marked implemented. No code, Git commits, remote pushes, issue closure or cross-project deployment is part of this turn.

## Ownership and unresolved adoption

- Git-FS owns candidate content, identity, safe mutations and publication/recovery contracts.
- Overwatch owns plan/claims, check selection and execution, result storage and acceptance orchestration.
- Tailscale/hub owns authorized publication routing, generations, catalog comparison and client refresh diagnostics.

- No dedicated Git-FS publication is exposed in this chat; only language-1c1d is available.
- A Unix execution target has not been discovered; native Unix acceptance remains a future gate.
- The entire .overwatch directory is currently ignored by Git; the plan is saved in the local project ledger, not committed or synchronized. Preserve the existing ignore decision; a future publishing task must explicitly choose how vault notes travel.
- No full test suite is rerun for this planning task; prior a39422c evidence is not evidence for future implementation.
- Paper findings guide design; their guarantees are not automatically inherited by Git-FS.
