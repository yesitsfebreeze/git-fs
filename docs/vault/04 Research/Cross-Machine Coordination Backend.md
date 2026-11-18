# Cross-Machine Coordination Backend

Planning research and proposed backend contract, 2026-11-18. This note records source evidence and design constraints; it is not implementation, test, deployment, or acceptance evidence.

Related plan: [[Cross-Machine Coordination Backend.prd|Cross-Machine Coordination Backend]]. System owner: the Overwatch repository's `Cross-Machine Git-fs Collaboration` PRD and `02 Wiki/Git-fs Collaboration Protocol.md`. Those are an external adoption dependency, not a local `needs` entry or a path this repository may edit.

## Evidence and limits

The inspected committed Rust source is `d8dd7b559ae10cc27880d360d0f2aecfbb53d859`, package 4.4.0. The parent Codex git-fs MCP reports build `a830787f65119e36459af5c27a7609cfe3ad2397`; the installed Claude-cache and target/release executables inspected here report older `89ab49b03eed1b80fcca6df1dbc0c3396afa6264`. The relevant source comparison to a830787f changes tombstone discovery in store.rs, not the inspected coordination/publication mechanisms. Runtime identity must always name the executable actually serving requests.

| Existing boundary | Source evidence | Consequence |
| --- | --- | --- |
| Local shared state | README.md and docs/reference.md explicitly limit shared live commits to one repository | Separate clones do not share live refs merely because both expose MCP |
| Safe publication | src/store.rs:1481, publish_live_with_intent; src/store.rs:349, mutation_lock | Reuse the repository lock, preimage recheck, three-way merge and expected-old-value ref update |
| Standard MCP binding | src/mcp.rs:81-189, BoundTarget and Bindings | A standard stdio backend serves one caller; gateway labels cannot establish authenticated isolation |
| Current MCP surface | src/mcp.rs:238-304 | Tools only; no event subscriptions or replication API |
| Existing evidence shape | src/change_record.rs; src/provenance.rs:163-185 | Recorder::Absent publishes nothing; provider-reported evidence never grants authority |
| Partial publication | src/files.rs:308-341 | Session commit can succeed before shared-live publication fails; outcomes must preserve that distinction |
| Snapshots/materialization | docs/reference.md; src/store.rs:1866; src/snapshot.rs | Pin builds and protect foreign disk/index changes; session metadata is host-local |
| Release | src/integrate.rs, src/push.rs, docs/integration-handoffs.md | Reuse guarded review, exact candidates, operation identity, handoff and reconciliation |

The existing Live Shared Agent Commits PRD explicitly leaves continuous cross-machine replication outside its accepted local contract. Reliable Concurrent Git-FS Contracts already owns session identity, verification and recovery; this work extends those owners. Git-FS Speed Responsiveness and Parallelism owns local scheduling/search behavior. Git-FS Native Mosaic Port freezes a830787f as a parity oracle and must keep that baseline unchanged.

No full suite, distributed fault test, installed-host mutation or two-machine acceptance was executed for this research. Slow read-only discovery on the older 89ab executable was cancelled by its owning research session; no peer process or lock was removed.

## Proposed consistency model

One configured project authority accepts shared changes. Independent agents and machines may compute and build concurrently; the brief authority checks and Git ref publication are serialized. Replica freshness is explicit: the applied revision and currently observed authority revision may differ.

Overwatch owns authentication, project registration, lease lifecycle and takeover intent. Git-fs owns the durable installed fence, source freshness and merge decisions, publication linearization and operation outcomes. A claim-change event informs a subscriber; it does not authorize a write.

No automatic authority failover or offline shared writes are included. A disconnected machine may read or build an existing pinned snapshot. It must reconnect and reconcile before obtaining a new shared claim or publishing accepted state.

## Versioned capability and identity

Advertise the coordination extension separately from the existing tools. Its capability result identifies the schema version, canonical repository binding, configured project and authority identity, current installed authority generation, supported mutation coverage, object format and runtime build/schema fingerprint.

An authenticated host binds a caller to a session. Model arguments cannot substitute a different actor, root, project or authority. A standard stdio deployment uses a distinct backend/session per remote caller unless the serving implementation has a tested trusted multiplexing contract. Coordinated mode refuses all unsupported mutation paths rather than quietly falling back to uncoordinated behavior.

All external fields are bounded and validated; paths remain repository-relative within the existing confinement rules. Event payloads never select executables, arbitrary filesystem roots, remote URLs or unregistered refs.

## Fence installation and mutation acceptance

A fence record binds project/repository, authority generation, claim id and generation, allowed scope, actor and current lifecycle state. Generation order is authority-controlled; wall-clock timestamps do not decide ownership.

Installation/advance is an idempotent operation with an expected prior fence. Under the same git-fs repository mutation lock used for shared publication, validate the existing record, retain the new durable fence, and return its exact resulting revision. A stale or conflicting advance refuses. Overwatch acknowledges takeover only after that backend acknowledgement is proved.

Every coordinated mutation validates the installed fence and permitted scope at its protected publication boundary while holding that lock. Settlement is limited to the fence scope being reassigned and its actual unsettled source or delivery effects; unrelated claims, replica imports and pinned builds remain concurrent. A completed shared-live compare-and-swap with a missing notification is settled source state. A check performed before acquiring the lock cannot substitute. The coverage inventory includes write, edit, delete, capture, rollback, live reconciliation, integration, undo and guarded push; any omitted surface keeps activation blocked.

Keep bulk object transfer, network waiting and builds outside the short authority mutation critical section. Bounded preimage recheck, merge and hashing of the resulting accepted blob may remain under that lock when required for a correct compare-and-swap; do not split the transaction based on an unmeasured performance assumption.

An external push already dispatched is a special case: a newer fence cannot revoke bytes already sent to Git. Record dispatch before launching, and hold takeover acknowledgement until the old operation is quiescent or authoritatively reconciled. Lost acknowledgements remain unknown; do not issue a fresh push operation to make progress.

This is a contract for coordinated git-fs operations, not an OS sandbox against an authorized user running arbitrary Git commands. Existing hooks remain workflow aids.

## Operations and recovery

Bind each durable operation to the authenticated caller, stable operation id, intent digest, installed fence, expected source revision and permitted effect. Reusing an id with different intent refuses. Keep enough source/ref/commit evidence to reconcile after restart without trusting the submitting client.

Observable outcomes include `not_dispatched`, `session_only`, `shared_accepted`, `unknown`, and refused intent, with stage-specific disk, integration and remote-publication evidence. A session commit never implies shared acceptance. Unknown outcomes require inspection of the recorded original target and actual refs; they never imply permission to redispatch.

Persist intent before a non-repeatable effect and retain conservative outcome evidence across interruption. Define exactly which process-crash guarantees apply; do not invent a power-loss or globally exactly-once guarantee beyond the selected storage protocol. Operation retention must not remove an unresolved receipt or make its id dispatchable again.

## Accepted-state query and notifications

The authoritative query reports project/authority binding, installed generation, accepted shared-live commit/tree, relevant operation outcome, and a revision token that detects a changed observed state. It must describe one consistent observation or explicitly report an incomplete view.

Notifications such as `source.changed` or `claim.changed` are bounded invalidation hints. They may be duplicated, delayed, reordered or dropped. Overwatch periodically reconciles the authoritative state even after a quiet connection, and queries again after reconnect or generation change. A missing hint therefore cannot silently strand a replica.

No generic durable event broker is required. Existing Git refs/history and durable operation receipts remain the evidence of publication. A client that needs history beyond available ancestry must obtain a fresh verified snapshot; a guessed sequence or source SHA is never sufficient.

## Replica import and local execution

Transfer immutable Git objects through a configured authenticated Git transport. Validate project/authority binding, object format, object closure and the exact advertised revision before acknowledging a replica. Import into an allowlisted replica namespace and advance only with expected-old-value checks.

Replica import must not rewrite reviewed main, local shared-live or session refs, or reinterpret foreign session identities as local authority. Git-fs owns the API that allows its local read/snapshot view to select an accepted replica revision; simply fetching an unused ref does not make current read/search precedence converge.

Create build snapshots pinned to that exact revision, with local ownership and release capabilities. Incoming changes cannot retarget a running build. Preserve raw bytes/modes/deletions or refuse unsupported filesystem transformations. Guarded materialization is optional and explicit; local disk differences are retained and reported.

Never replicate the .git-fs directory wholesale. Read freshness records, indexes, locks, materialization ledgers, credentials, operation authority, snapshot paths and capabilities are local state, even when source revisions match.

## Compatibility and ownership

Implement against the currently accepted backend owners in src/, with executable-neutral extension fixtures in tests/. If Mosaic becomes the serving implementation, it must advertise and pass the same extension fixtures before coordinated activation. This does not make the entire Mosaic port a prerequisite, change its frozen oracle, or certify a runtime merely because it reports version 4.4.0.

Remote write freshness is established by reading the current path through the caller's bound authoritative git-fs session before submitting an edit. A read from a local replica or another caller does not populate that session's freshness records. Re-read and reconcile after reconnect, a changed base or a freshness refusal; never synthesize authority read evidence from replica hashes.

Execution and delivery ownership are separate. After unit_record releases execution ownership, Overwatch obtains an explicit delivery lease with unit_load {delivery:true}; integrate, undo and push use that lease's current scoped fence. An expired execution claim cannot authorize a later release.

Overwatch's adopter must test against exact backend capabilities and source builds. It owns event routing, periodic reconciliation, replica status, lease transitions and deployment orchestration. This repository must not write ../overwatch paths or pretend an external contract is a completed local unit.

Disable coordination through a documented quiesce/reconcile step. Preserve accepted commits, private attempts, unresolved receipts and existing stores; retaining a previous executable does not authorize it to publish against a newer installed fence. Legacy runtimes that cannot enforce the extension must remain excluded from the coordinated write endpoint and its authoritative store. Overwatch capability negotiation alone cannot prevent an older binary from directly mutating the same local repository. Before enable, prove incompatible-store refusal using the actual older executable, or use an exclusively isolated authoritative repository exposed only through managed access; if neither is established, refuse coordinated activation. Preserve existing drafts and clients rather than killing peers or deleting locks. This scope does not claim to fence an administrative user running arbitrary Git; detect such divergence and pause managed publication.

## Evidence stages and noncircular adoption

1. A backend candidate advertises the versioned extension and passes its own contract fixtures; this makes it eligible for isolated integration testing, not production activation.
2. Candidate Overwatch and backend builds may run isolated cross-repository and two-machine fixtures before either PRD is finally accepted. Keep those fixtures separate from the production authority and identify both exact executables.
3. Backend acceptance depends on its own native Windows/Unix fixtures, complete mutation coverage, exclusion of incompatible authority-store clients, recovery/rollback evidence and the versioned handoff. It does not depend on final acceptance of the Overwatch parent PRD.
4. Parent production activation and final joint acceptance require that accepted backend plus the parent route, lease, replica and real-machine evidence. An incomplete parent rollout does not retroactively make already-proved backend conformance circular.

## Execution order and final proof

Unit labels are stable identifiers, not execution order. After the declared B01-B16 prerequisites, execute B19 (legacy-client exclusion), then B20 (lifecycle preparation and rehearsal), then B17 (final native Windows acceptance), then B18 (final native Unix acceptance and versioned handoff). B19 runtime edits and B20 fixture edits therefore precede both final platform suites. B20 defines the later acceptance requirements and never requires their receipts to finish its own implementation.

Both native receipts must cover the finalized runtime, fixtures and build inputs. Any later runtime, fixture or build-input change invalidates both platform receipts and requires both suites to rerun before final handoff. A documentation-only receipt commit may reuse native proof only with explicit source/tree-equivalence evidence for all runtime, fixture and build inputs. B18 records the final versioned backend handoff after both current platform receipts pass; no parent final-acceptance receipt is a backend prerequisite.

## Required proof

Extend existing tests rather than replace their contracts: tests/concurrent_workflow.rs, tests/session_routing.rs, tests/unit/git_mode.rs, tests/verification_snapshot.rs, tests/integration_recovery.rs and tests/guarded_push.rs. New extension fixtures must exercise two repositories and real independent processes.

Test disjoint edits, same-file nonoverlap, conflicting edits, wrong identity/scope, fence advance racing publication, duplicate and changed operation ids, session-only failures, crashes before/after each effect, lost notifications, disconnected replicas, missing objects, malformed refs, stale source revisions, conflicting hand edits, pinned builds and takeover during an unknown push. Assert actual refs, bytes, histories, receipts and surviving foreign state, not only success messages.

Native Windows and Unix runs identify exact executable hashes and source revisions. Final real two-machine activation requires both this backend PRD and the Overwatch adoption contract to pass; all implementation acceptance boxes remain unchecked until that evidence exists.
