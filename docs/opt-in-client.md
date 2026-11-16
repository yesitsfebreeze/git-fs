# Opt-in fresh Git-FS MCP client (Windows)

This is an **explicit new-client preview**, not an upgrade of installed plugins,
their hooks, the authenticated Tailscale hub, or any running MCP process.

The verified release artifact on FEB is:

- Git source: `8d83c2cd265eb59ca6c91f45d67178cc7fb50920`
- Binary: `C:\Users\sayhe\dev\claude-plugins\git-fs-plugin\bin\versions\8d83c2cd265eb59ca6c91f45d67178cc7fb50920\git-fs.exe`
- SHA-256: `1980E4D2041B8718D0B64386C7C897B56C2F0ACE464077AE9A82C2D0359C57D0`

`stage-runtime.ps1` builds and verifies a native copy under `bin/versions`.
`opt-in-client.ps1` verifies the exact executable SHA-256 when generating a
config and during native smoke, and validates
the plugin and workspace paths, and refuses to overwrite an existing config.
The generated config sets `GIT_FS_COORDINATED=1`, `GIT_FS_SESSION`, and
`GIT_FS_DISK` only for the new native child MCP process. It does not persist credentials or alter global
Claude/Codex MCP registration, installed plugin binaries, hooks, or services.

## Test a fresh independent native process

From PowerShell **on FEB** (replace workspace with the intended repository):

```powershell
$script = 'D:\dev\git-fs\scripts\opt-in-client.ps1'
$plugin = 'C:\Users\sayhe\dev\claude-plugins\git-fs-plugin'
$revision = '8d83c2cd265eb59ca6c91f45d67178cc7fb50920'
$sha256 = '1980E4D2041B8718D0B64386C7C897B56C2F0ACE464077AE9A82C2D0359C57D0'
$workspace = 'D:\dev\git-fs'
$session = 'gfs10-' + [guid]::NewGuid().ToString('N')
powershell.exe -NoProfile -File $script -Smoke -PluginRoot $plugin -Revision $revision -Sha256 $sha256 -Workspace $workspace -Session $session
```

A passing smoke test launches a disposable Git-FS server, checks MCP
`initialize`, `tools/list`, the candidate snapshot contract
(`kind: candidate`, `expected_tree`, `expected_review`, `capability`),
`integrate.retry_sync`, and `gitfs_ls` for that session. It then stops only
its disposable child.

## Opt a new Claude Code process into that pinned MCP

```powershell
$config = Join-Path ([IO.Path]::GetTempPath()) ($session + '.mcp.json')
powershell.exe -NoProfile -File $script -PluginRoot $plugin -Revision $revision -Sha256 $sha256 -Workspace $workspace -Session $session -Output $config
claude --mcp-config $config --strict-mcp-config
```

This new process launches the immutable versioned EXE directly from an explicit
per-session JSON config. The generator checks its SHA-256, but Claude does not
re-hash it on every future launch: rerun the smoke and regenerate the config
when updating the source revision. A later process must use a **different**
`$session` and output path. Review the tool list
inside the client; do not treat the pre-existing hub publication as updated.

**Important boundaries:**

- `--strict-mcp-config` controls MCP configuration sources; it does **not**
  prove that a pre-installed `git-fs@4.4.0` plugin or its hooks are disabled.
  Do not rely on this MCP-only preview for protected Write/Edit interception.
- Do not register it as the global `git-fs` plugin or reload/repoint the
  installed manifest/hooks. A separate immutable plugin-wrapper release
  must pin **both** MCP and all hooks before new full editor sessions use it.
- Session IDs in a local config are caller-selected. They are **not**
  authenticated or revocable OAuth identities. Tailscale hub authorization,
  stable principal-to-session mapping, gateway trust, reconnect and catalog
  adoption remain independent acceptance gates.
- Roll back a new preview by ending its process; the 12 observed old Git-FS
  MCP processes continue on their previous executable.
