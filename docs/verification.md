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
