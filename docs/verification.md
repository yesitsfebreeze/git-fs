# Exact candidate verification snapshots

`gitfs_snapshot {}` retains the legacy live-ref snapshot behavior. Use
`gitfs_snapshot {"kind":"candidate"}` to verify the **merged integration
candidate**, not `refs/gitfs-live/current`, before a reviewed integrate/push.

The candidate uses the same `Store::reviewed_tree(session)` computation as
`integrate`: current repository/branch/HEAD, index revision, session overlay,
owned paths and final Git tree. The result binds all of those identities in
`source`, and reports the exact `tree` and a synthetic detached `revision`.
Pass `expected_review` (the full `source` object from a prior integration
preview) and/or `expected_tree` to refuse an intervening change. A new preview
is required after any source mismatch. This is not yet a check receipt; GFS-04
must bind checks to this exact candidate and validate freshness at publication.

The returned `path` is a separate, clean, detached Git worktree. Its tracked
regular-file bytes are verified against the candidate tree blob IDs with Git
filters disabled; staged Git modes are verified too. On Unix the executable
filesystem bits are checked. On Windows, executable mode is guaranteed in Git
metadata, not by Windows filesystem permissions. A later peer edit or trunk
advance never changes an existing candidate checkout.

Each creation returns `id`, a 256-bit `capability`, `manifest`, `source`,
`tree`, `revision` and `path`. Treat the capability as a per-run secret; only
its hash is stored in the ownership manifest. To release:

```json
{"kind":"candidate","id":"<returned id>","capability":"<returned capability>"}
```

The same session with a different run capability cannot release it. Cleanup
checks exact owner, repository, checkout location, HEAD, tree and all changes,
including ignored/untracked outputs. It never uses forced worktree removal.
If checks wrote inside the candidate or an interrupted operation left a
partial checkout, inspect and recover it explicitly; the manifest remains
where cleanup was unsafe. Write mutable build/test artifacts outside the
candidate checkout, with separate directories per check run.

This is an **isolated checkout, not a hermetic sandbox**. Tools and scripts
may still access external paths, credentials, environment variables, services,
network and mutable dependencies. Verification must record those limitations.
Symlinks, submodules/gitlinks, non-UTF-8 paths and unsupported special paths
are rejected. Any checkout/smudge/filter/EOL conversion that changes raw blob
bytes is also rejected; configure lossless checkout semantics before retry.

Acceptance checks for this contract:

```sh
cargo fmt --all -- --check
cargo test --locked --test verification_snapshot
cargo test --locked
```

## Candidate-bound command verification (GFS-04)

Git-FS consumes receipts from a separately configured, host-owned check runner;
it does **not** schedule checks, run an agent, or accept approval JSON in an MCP
tool argument. Configure `gitfs.verificationPolicy` in repository-local Git
config as a versioned JSON policy such as:

```json
{"schema":"gitfs-verification-policy/v1","runner_id":"overwatch-runner","receipt_dir":"D:/trusted-gitfs-receipts","checks":[{"id":"unit","definition":"cargo test --locked","required":true}],"environment_allowlist":[],"reuse":"exact-candidate"}
```

The absolute receipt directory must exist **outside the repository** and must
be writable only by the trusted runner/store. The runner should create a fresh
`gitfs_snapshot {"kind":"candidate"}` with the exact source object from the
current integration preview, run the declared checks inside that detached
checkout, record actual stdout/stderr, and place mutable build outputs outside
the candidate checkout. Each run uses distinct `run_id`/revision-keyed artifact
filenames to prevent one parallel session from overwriting another's logs.

The runner deposits `<review_revision>.json` in the configured receipt store,
with schema `gitfs-verification-receipt/v1`, canonical `preview` and matching
`source`, `session`, `run_id`, runner ID/revision, `review_revision`, policy and
check-definition SHA-256 digests, platform, toolchain, dependency versions or
digests, environment allowlist, limitations, and one record per declared check.
Each check records its ID, SHA-256 of its exact configured definition, status
`passed`, `input_integrity: unchanged`, SHA-256 and absolute trusted-store
paths of stdout/stderr, plus explicit `output_digests`. Files must remain
bounded regular files in that directory and unchanged when later verified.
The full canonical preview must hash to the review revision, including subject,
author, source repository/branch/HEAD/index/overlay/tree and policy fingerprint.

A configured policy fails closed on missing, incomplete, failed, cancelled,
timed-out, stale, cross-run or modified evidence. All configured checks are
currently required; `required: false` is rejected rather than silently
granting optional-check semantics. Publication independently revalidates the
receipt before the commit transaction and in both Git-FS push range checks and
`gitfs review-check`, even when no command-review policy is configured.
Existing secret checks, model gate and external command-review continue to run.

**Trust boundary:** repository-local configuration and files are not
cryptographic attestation. Same-UID or filesystem writers that can modify the
trusted receipt directory or Git config can forge evidence; control these with
host ACLs and isolated runner identities. A candidate checkout is not an
execution sandbox. The current `check_range` API receives Git old/new OIDs but
not the destination ref; the checked preview records the source branch, while
destination-ref enforcement remains the native push adapter's responsibility.
