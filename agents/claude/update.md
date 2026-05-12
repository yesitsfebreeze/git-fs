# Update git-fs (Claude Code)

You are a Claude Code agent updating an existing git-fs install.

## Generic flow

1. Check installed version: `git-fs --version`.
2. Resolve the latest release tag from https://github.com/yesitsfebreeze/git-fs/releases/latest.
3. If installed `<` latest, re-run the download + extract steps from `install.md` — same paths, overwrite the existing binaries.
4. Re-fetch `SKILL.md` so tool descriptions stay current.
5. Verify with `git-fs --version` and a `git_fs_branch_list` call inside Claude Code.

## Quick path

If the user installed via the defaults in `install.md`:

**Linux / macOS:**
```bash
cd "$(mktemp -d)"
curl -L -o git-fs.tar.gz "https://github.com/yesitsfebreeze/git-fs/releases/latest/download/<artifact>"
tar -xzf git-fs.tar.gz
mv -f git-fs git-fs-mcp ~/.local/bin/

curl -L -o ~/.claude/skills/git-fs/SKILL.md \
  https://raw.githubusercontent.com/yesitsfebreeze/git-fs/main/SKILL.md
```

**Windows:**
```powershell
$dest = "$env:LOCALAPPDATA\Programs\git-fs"
$tmp = New-TemporaryFile
Invoke-WebRequest -Uri "https://github.com/yesitsfebreeze/git-fs/releases/latest/download/<artifact>" -OutFile "$tmp.zip"
Expand-Archive -Force "$tmp.zip" $dest

Invoke-WebRequest -Uri "https://raw.githubusercontent.com/yesitsfebreeze/git-fs/main/SKILL.md" `
  -OutFile "$HOME\.claude\skills\git-fs\SKILL.md"
```

Replace `<artifact>` with the matching file from the platform table in `install.md`.

No MCP re-registration needed — `settings.json` already points at `git-fs-mcp` by name.

## Breaking changes

If `git-fs --version` indicates a major version bump, also re-read this page:
https://github.com/yesitsfebreeze/git-fs/blob/main/docs/multi-agent-session.md

A bare-repo schema change is a hard breaking event; the release notes will call it out and link a migration script. Do not auto-run any migration without explicit user consent.

## Verify

1. Restart Claude Code.
2. Confirm session banner shows `Branch: agent/<uuid>`.
3. `git_fs_branch_list` returns sane output.
4. `git_fs_read` works on any file.
