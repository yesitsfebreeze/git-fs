# Install git-fs (Claude Code)

You are a Claude Code agent installing git-fs for a user. Follow this flow end-to-end. Stop and ask the user only when explicitly told to.

## Generic flow

1. Detect platform (OS + architecture).
2. Resolve the latest release tag.
3. Download the matching binary archive.
4. Extract `git-fs` + `git-fs-mcp` into a stable location that's on PATH.
5. Install `SKILL.md` to the user's skills directory.
6. Register the MCP server in Claude Code settings (and optionally the hooks).
7. Bootstrap any specific project the user names.
8. Verify.

## Step 1 — Detect platform

| OS | Arch | Artifact |
|----|------|----------|
| Linux | x86_64 | `git-fs-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | aarch64 / arm64 | _not yet released — build from source: `cargo build --release -p git-fs`_ |
| macOS | Intel | `git-fs-x86_64-apple-darwin.tar.gz` |
| macOS | Apple Silicon | `git-fs-aarch64-apple-darwin.tar.gz` |
| Windows | x86_64 | `git-fs-x86_64-pc-windows-msvc.zip` |

Use `uname -sm` (Unix) or `$env:PROCESSOR_ARCHITECTURE` (Windows) to detect.

## Step 2 — Latest release

Latest release index: https://github.com/yesitsfebreeze/git-fs/releases/latest

If the user gave no specific version, use `latest`. Archive URL pattern:

```
https://github.com/yesitsfebreeze/git-fs/releases/latest/download/<artifact>
```

## Step 3 — Install location

Pick a location that is on PATH and persists across reboots.

- **Linux / macOS:** `~/.local/bin/` (most common; usually already on PATH).
  If `~/.local/bin` is not on PATH, instruct the user to add it to their shell rc.
- **Windows:** `%LOCALAPPDATA%\Programs\git-fs\`. Add it to user PATH via:
  ```powershell
  [Environment]::SetEnvironmentVariable("Path", $env:Path + ";$env:LOCALAPPDATA\Programs\git-fs", "User")
  ```
  Tell the user to open a fresh terminal for the PATH change to take effect.

## Step 4 — Download + extract

**Linux / macOS:**
```bash
mkdir -p ~/.local/bin
cd "$(mktemp -d)"
curl -L -o git-fs.tar.gz "https://github.com/yesitsfebreeze/git-fs/releases/latest/download/<artifact>"
tar -xzf git-fs.tar.gz
mv git-fs git-fs-mcp ~/.local/bin/
chmod +x ~/.local/bin/git-fs ~/.local/bin/git-fs-mcp
```

**Windows:**
```powershell
$dest = "$env:LOCALAPPDATA\Programs\git-fs"
New-Item -ItemType Directory -Force $dest | Out-Null
$tmp = New-TemporaryFile
Invoke-WebRequest -Uri "https://github.com/yesitsfebreeze/git-fs/releases/latest/download/<artifact>" -OutFile "$tmp.zip"
Expand-Archive -Force "$tmp.zip" $dest
```

Confirm: `git-fs --version` and `git-fs-mcp --help` (the latter may print a server banner — that's fine).

## Step 5 — Install the skill

```bash
mkdir -p ~/.claude/skills/git-fs
curl -L -o ~/.claude/skills/git-fs/SKILL.md \
  https://raw.githubusercontent.com/yesitsfebreeze/git-fs/main/SKILL.md
```

Windows:
```powershell
$skill = "$HOME\.claude\skills\git-fs"
New-Item -ItemType Directory -Force $skill | Out-Null
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/yesitsfebreeze/git-fs/main/SKILL.md" `
  -OutFile "$skill\SKILL.md"
```

## Step 6 — Register the MCP server

Add to `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "git-fs": { "command": "git-fs-mcp" }
  }
}
```

If `mcpServers` already exists, merge — do not overwrite other entries.

### Optional: enable the full git-fs workflow (recommended)

For automatic session-branches + Stop-merge, also add:

```json
{
  "permissions": { "deny": ["Edit", "Write"] },
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "git-fs hook session-start" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "git-fs hook stop" }] }
    ],
    "PreToolUse": [
      { "matcher": "Read", "hooks": [{ "type": "command", "command": "git-fs hook read" }] }
    ]
  }
}
```

`deny: ["Edit", "Write"]` is intentional: it forces all writes through git-fs MCP tools. That's the whole design. Without it, edits bypass the audit trail.

If the user wants this only on specific projects (not globally), put the `hooks` + `permissions` block in `<project>/.claude/settings.json` instead of `~/.claude/settings.json`.

## Step 7 — Bootstrap a project (optional, project-scoped)

If the user names a target project directory, bootstrap it:

```bash
cd /path/to/project
git-fs init-project
```

This creates `.git-fs/` (the bare repo) and writes a project-local `.mcp.json` so the git-fs MCP is loaded for that project.

## Step 8 — Verify

1. Restart Claude Code (full quit + reopen).
2. Open a project where git-fs is registered.
3. Confirm the session banner contains `Branch: agent/<uuid>`.
4. Ask Claude Code to call `git_fs_branch_list` — it should return at least the current agent branch and `main`.
5. Ask Claude Code to read a file via `git_fs_read` — should succeed.

## Troubleshooting

- **`git-fs: command not found`** — install dir not on PATH. Add it; open a new terminal.
- **No `Branch:` banner** — `SessionStart` hook not registered, or `git-fs` not on the hook subprocess's PATH. Use an absolute path in `settings.json` if needed.
- **MCP not loading** — confirm `git-fs-mcp` runs in a normal shell. Restart Claude Code completely.
- **Edits still hitting disk** — `permissions.deny` not applied. Reload settings; check spelling.
