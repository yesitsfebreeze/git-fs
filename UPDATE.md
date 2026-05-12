# Update git-fs

Updating means refreshing two things:

1. **Binaries** (`git-fs`, `git-fs-mcp`) on `PATH` — replace with the latest release.
2. **`SKILL.md`** inside your agent's skills directory — so tool-selection rules track the binary.

MCP registration and hooks usually do not need to change between versions; if a release requires it, the release notes will say so.

## Pick your agent

| Agent | Guide | Prompt to paste |
|-------|-------|-----------------|
| Claude Code | [`agents/claude/update.md`](agents/claude/update.md) | `Read https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/update.md and update git-fs.` |

> Don't see your agent? See [Adding a new agent](INSTALL.md#adding-a-new-agent) in the install guide.

## Generic flow

Every per-agent guide follows the same shape:

1. `git-fs --version` — read current version.
2. Resolve latest tag from https://github.com/yesitsfebreeze/git-fs/releases/latest.
3. If `installed < latest`, download the matching archive for your platform.
4. Overwrite the existing `git-fs` + `git-fs-mcp` binaries in their install location.
5. Re-fetch `SKILL.md` so tool descriptions stay in sync with the binary.
6. Restart the agent. `git-fs --version` again to confirm.

## Manual update

```bash
# fetch latest archive for your platform from /releases/latest, then:
tar -xzf git-fs-<target>.tar.gz
mv -f git-fs git-fs-mcp ~/.local/bin/

# refresh skill (Claude Code path; other agents differ)
curl -L -o ~/.claude/skills/git-fs/SKILL.md \
  https://raw.githubusercontent.com/yesitsfebreeze/git-fs/main/SKILL.md
```

Restart the agent.

## Breaking changes

A major-version bump (`v1.x → v2.x`) may include a bare-repo schema change. The release notes will call it out and link a migration script. Do not run any migration without consenting.
