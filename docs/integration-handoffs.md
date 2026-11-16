# Git-FS native adoption handoffs

This is a handoff contract, not proof that an external client or Tailscale
gateway has deployed the native Git-FS changes. The native acceptance suites
provide core behavior evidence; external installs require separate claims.

## Overwatch consumer adapter

- Owner to resolve: Overwatch consumer owner for external D:/dev/overwatch.
  Confirm its remote, branch, HEAD, worktree, installed executable, active
  claims and MCP schemas before adoption.
- Required versions: JSON-RPC 2.0, MCP 2025-06-18, Git-FS runtime identity
  schema_version 1; record tool_schema_profile and tool_schema_fingerprint.
- Request fixture: initialize a new connection for every trusted caller;
  gitfs_ls must report the bound runtime.session.id and canonical cwd. Read
  before edit/delete; preview an integration, then commit only its returned
  expected_revision. Reconcile an ambiguous operation ID after restart.
- Receipt fixture: outcome.schema gitfs-integration-outcome/v1, exact
  operation_id, committed SHA and tree, publication, synchronization and
  next_action. The client must not interpret push:false as remote delivery.
- Acceptance: two distinct callers on one repository, independent session
  refs and Git blobs, nonoverlap integration, stale same-line refusal,
  replay/reconcile producing the SAME commit and preservation of unrelated
  staged and unstaged changes. Run under the consumer's own Overwatch claim.

Example MCP calls (session/root identity is bound by the trusted process/host):

    {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"adopter","version":"1"}}}
    {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"gitfs_ls","arguments":{}}}
    {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"read","arguments":{"path":"example.txt"}}}
    {"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"integrate","arguments":{"phase":"preview","message":"example"}}}

## Authenticated hub capability diagnosis

- Owner to resolve: Overwatch/Tailscale gateway owner, coordinated with the
  separate agent-independent wake/identity unit and its protected claim.
  Native stdio GIT_FS_SESSION acceptance is not two-OAuth-principal proof.
- Discover the current authorized publication, source route, backend health,
  loaded revision and catalog generation. Observed on FEB: git-fs-65bc,
  D:/dev/git-fs, but names and generations can change.
- Compare exactly one authorized publication across installed binary, direct
  backend MCP tools/list, hub catalog and actual client-visible schemas.
  Record source SHA, schema fingerprint, timestamp, tool name/args and page
  cursor for each surface. Paginate fully.
- Add/change a controlled publication and verify subsequent discovery; test
  server notifications separately from ChatGPT client refresh. Treat a
  catalog that cannot be refreshed as STALE DISCOVERY, not success.
- Test two authenticated principals with distinct connections, reject
  client-supplied identity spoofing and verify independent Git refs/bytes.
- External status: pending gateway owner, claim and OAuth test identities;
  this cannot be closed by native Git-FS tests.

Observed on 2026-11-16 before GFS-08: authenticated hub publication
git-fs-65bc, hub catalog revision f9eb823b036a74428fb0c7a8d631fac93c7701bde26b26c1d0df42e22cf59ccf,
git-fs__gitfs_snapshot schema exposed only id/items/_overwatch,
not candidate kind/expected_tree/expected_review/capability. The native
MCP validator likewise rejected candidate arguments (job mwik5fa3:
4/5; fifth failed before snapshot handler). This is a schema/dispatch
gap, not a snapshot-store failure; recheck deployed hub and ChatGPT
client generations after GFS-08, and report stale discovery until then.

## Native acceptance and platform reproduction

Use the same committed source tree and same policy content on both platforms.
An old Linux clone or Windows zero-test review_gate does not establish Unix
coverage. Run these in isolated checkouts:

    cargo fmt --all -- --check
    cargo clippy --all-targets --locked -- -D warnings
    cargo test --locked --test concurrent_workflow --test runtime_identity --test session_routing --test verification_snapshot --test verification_receipt --test integration_recovery --test guarded_push --test tool_items
    cargo test --locked
    cargo build --release --locked

Additionally on Unix run cargo test --locked --test review_gate explicitly.
The pre-push test fixture must invoke the production hook with genuine Git
argv and GITFS_PUSH_USER_HOOKS / GITFS_PUSH_REMOTE; never weaken the hook.
Record OS, rustc/cargo versions, commit/tree SHA, check-policy digest,
test counts, exclusions and failures. WSL through Git Bash needs
MSYS_NO_PATHCONV=1 for Unix argv; use /home/sayhe/.cargo/bin/cargo,
CARGO_INCREMENTAL=0, fresh Linux-local target and --jobs 2.

Pre-GFS-08 staged tree 3daafc89c775eed6f1a6434961ee59b9b322235e had
identical check-policy inputs on Windows and Ubuntu: justfile Git blob
b6169113d69c23aea9a672a64585b58add8c1726 and Cargo.lock Git blob
863636809174385827b10069a2fdb95f8321035b.

The native process-restart scenario restarts after the session ref was
published but before integration. The separate GFS-05 integration_recovery
suite covers post-Git-ref/pre-ACK recovery, including exact receipt replay.

Fresh-process protocol behavior is exercised separately by runtime_identity:
initialize_negotiates_only_implemented_revisions and
malformed_initialization_params_are_rejected. The two-peer harness
initializes exactly once; duplicate initialize is not a valid negative.

Formatter compatibility caveat (source tree 3daafc89): Windows rustfmt 1.10
passes repository-wide; Ubuntu rustfmt 1.11 proposes formatting changes in
untouched src/mcp.rs and tests/unit/integrate/tests.rs. Ubuntu's acceptance
test file formats cleanly, and strict Clippy passes. Pin/harmonize the
formatter separately before claiming Unix repository-wide fmt green.

## GitHub issue 1 evidence boundary

Issue #1 (plain folders) remained OPEN on 2026-11-16. Current source
StoreConfig::at (src/store.rs) falls back to init_standalone, and
runtime_identity tests inspect an existing standalone store. These are
not end-to-end proof of native edit/write and hook routing from plain
folders containing nested repositories. Keep issue open pending that
separate direct-client acceptance.

## Non-gating performance snapshot

Fixture: three tracked text files, one base commit and two live MCP child
processes on the same checkout.

Windows pre-integration baseline: four native multi-process tests, job
mwil4vni, `cargo test --test concurrent_workflow -- --skip
two_peer_candidate_snapshots_are_exact_isolated_and_capability_scoped --nocapture`.
Across seven process lifetimes, 57 timed stdio MCP requests yielded median
256,990 us, 95th percentile 2,204,984 us, and max 3,400,012 us. Traces name
each RPC/peer and were printed by the bounded client harness.
There were three expected fail-closed replies (stale preimage, stale revision,
outside-root traversal), zero automatic conflict retries.
The candidate-snapshot case was excluded pending GFS-08's MCP contract fix;
record the final full Windows/Unix same-source runs separately.

Linux Ubuntu pre-integration on exact source tree 3daafc89c775eed6
(independently verified by Git write-tree job mwileaer), job mwiletam:
4/4 native tests; 57 timed stdio RPCs, median 14,646 us, p95 101,087 us,
max 102,918 us, three expected refusals, zero automatic conflict retries.
Measurements are non-gating diagnostics, not timing assertions; compiler-
inclusive test wall time is not per-RPC latency.

## Final same-source acceptance (2026-11-16)

Deterministic schedule points (`GITFS_TEST_BARRIER_DIR`, see the reference
Development section) now let `concurrent_workflow` replay two in-process
races by event order: a peer publishing the same line between A's handler
preimage check and its locked live re-check (A refused with `live conflict`,
never reaches `publish`), and a disjoint peer held behind A's locked
publication (both land, in trace order). Git inspects refs, blobs, HEAD and
disk afterwards.

Live commit 415ebe6d1076f2a66f04a083385b416a915c2db4, tree
cf690ba4833d6790e9a55321d97deb9c57a36edc. Source and policy blob lists
(src, tests, Cargo.toml, Cargo.lock, justfile, build.rs; blob ID + path)
hash to d01665694fab3578eff7fc8cf6d5bb2ae7f0404397c98394b435bbea91d49647 on
both machines; justfile blob b6169113d69c23aea9a672a64585b58add8c1726,
Cargo.lock blob 863636809174385827b10069a2fdb95f8321035b. The Linux
write-tree (d4ed92467afb) differs only by CRLF normalization of vault notes.

- Windows 11 26200, rustc/cargo 1.99.0: fmt, strict Clippy, the eight
  targeted suites (concurrent_workflow 7/7), full `cargo test` and release
  build all pass. review_gate runs 0 tests here; that is not Unix coverage.
- Ubuntu WSL2 6.18, rustc/cargo 1.100.0: strict Clippy, the eight targeted
  suites, review_gate 1/1, full `cargo test` (193 unit) and release build
  pass. `cargo fmt --check` FAILS only on the untouched src/mcp.rs and
  tests/unit/integrate/tests.rs (rustfmt 1.11 vs 1.10, caveat above);
  repository-wide Unix fmt stays a limitation until the formatter is pinned.

Windows non-gating snapshot, all seven native tests, 13 process lifetimes:
87 timed stdio RPCs, per-peer p50 400-1,430 ms, max 4,357 ms (a paused
barrier peer); seven expected fail-closed replies, zero automatic conflict
retries.

GitHub issue #1 and the Overwatch/hub adoption items above remain open; no
external post or close was made.
