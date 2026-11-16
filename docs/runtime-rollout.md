# Safe versioned Git-FS runtime staging

Stage a verified native executable for **future** clients without changing a live
Claude/Codex MCP server, hook, launcher, or stable executable. The agent-agnostic
Git-FS core remains independent of plugins.

## Why staging is separate from activation

The FEB directory-marketplace plugin currently launches
`C:\Users\sayhe\dev\claude-plugins\git-fs-plugin\bin\git-fs.exe` directly
from both its Claude MCP manifest and every hook. Its Codex wrapper also
forwards to that stable executable. Some existing sessions run the cached
`4.4.0` plugin binary instead. The installed binary hash and the verified
new Rust release hash differ; a source push or `/reload-plugins` does not
prove all clients run the new contract.

Never overwrite the stable executable, swap a running process, rewrite the
installed plugin manifest/hooks, or restart the shared Tailscale gateway as
part of this staging recipe. The existing `just build` / `install-plugin`
recipes are legacy entrypoints and must not be used to upgrade active
multi-client sessions.

## Stage and verify a pinned Windows release

From an isolated worktree whose Rust build inputs are clean:

```powershell
powershell.exe -NoProfile -File scripts/stage-runtime.ps1 -SelfTest

powershell.exe -NoProfile -File scripts/stage-runtime.ps1 -Repository D:\dev\git-fs-gfs07-20261116 -PluginRoot C:\Users\sayhe\dev\claude-plugins\git-fs-plugin -ExpectedRevision <exact-40-character-Git-HEAD>
```

The script:

1. Verifies an explicit Git HEAD and refuses modified/untracked Rust build
   inputs, arbitrary plugin roots, malformed plugin names, and reparse paths.
2. Runs `cargo build --release --locked` with the exact HEAD embedded as
   `GIT_FS_BUILD_REVISION`; computes SHA-256 of the executable.
3. Stages to `bin/versions/<HEAD>/git-fs.exe` through a unique same-directory
   temporary file and atomic **no-overwrite** rename. Matching repeated
   attempts succeed; a different binary at that revision is rejected.
4. Starts that exact staged executable in a disposable directory, negotiates
   MCP `2025-06-18`, and checks runtime build revision, pending binding,
   schema fingerprint and the `integrate.retry_sync` tool schema.
5. Rechecks SHA-256 of the staged binary and protected stable executable,
   plugin manifest, hooks and Codex launcher. It prints staged path, revision,
   binary SHA-256 and MCP schema fingerprint.

`-SelfTest` uses a disposable fixture to exercise exact-byte staging,
repeat idempotency and different-payload collision refusal; it does not
modify the installed plugin.

## Activation is an independent, coordinated handoff

The staged version is **not installed or activated**. To onboard new clients:

1. Own and publish the plugin wrapper/hook changes on the dedicated
   plugin-orphan branch, with a new immutable plugin version. Pin **all**
   MCP and hook entrypoints for that version to the same
   `bin/versions/<HEAD>/git-fs.exe`, including Codex. Do not change paths
   already held by running sessions.
2. Verify the new plugin's binary hash and its direct MCP schema/identity
   **before** allowing a new client to attach. The Tailscale catalog must
   report the adopted generation, not a cached older schema.
3. Prove two genuinely separate authenticated clients bind durable,
   revocable identities to distinct backend `GIT_FS_SESSION` contexts with
   coordinated mode enabled. A transient `hub:<UUID>` per invocation is
   not a durable client identity; do not trust caller-supplied routing fields.
4. Run GFS-06 cross-process Windows/Unix conflict/recovery acceptance and
   two-real-client hub tests, including reconnect, preserved live sessions,
   and rollback to the prior immutable plugin version if new-client
   adoption fails.

The authenticated gateway isolation handoff is separately tracked in
Overwatch and currently held behind a protected wake claim. Do not bypass
that claim or claim fully parallel authenticated clients from native tests.
