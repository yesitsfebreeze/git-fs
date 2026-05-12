# Install git-fs

git-fs installs in two pieces:

1. **Binaries** (`git-fs`, `git-fs-mcp`) on your `PATH` — these run the bare-repo, the MCP server, and the session hooks.
2. **Skill + MCP registration** inside your AI agent — so the agent knows the tool selection rules and can call the MCP server.

Your AI agent does the second piece for you. You just point it at the guide below.

## Pick your agent

Find your agent in the table and paste the prompt into it. The agent reads the guide, detects your platform, downloads the matching binary from the [latest release](https://github.com/yesitsfebreeze/git-fs/releases/latest), and finishes the wiring.

| Agent | Guide | Prompt to paste |
|-------|-------|-----------------|
| Claude Code | [`agents/claude/install.md`](agents/claude/install.md) | `Read https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/install.md and install git-fs.` |

> Don't see your agent? See [Adding a new agent](#adding-a-new-agent).

## What the agent does

Every per-agent guide follows the same generic flow:

1. **Detect platform.** OS + architecture.
2. **Download binaries.** Match against the [release artifact table](README.md#release-artifacts). Place `git-fs` and `git-fs-mcp` on a directory in `PATH`.
3. **Install the skill.** Copy [`SKILL.md`](SKILL.md) into the agent's skills directory so tool-selection rules are available next session.
4. **Register the MCP server.** Add an entry to the agent's MCP config so `git_fs_*` tools become callable.
5. **(Recommended) Register hooks.** `SessionStart`, `Stop`, and `PreToolUse(Read)` automate session-branching, auto-merge, and on-demand materialization.
6. **(Optional) Bootstrap a project.** `git-fs init-project` in a project directory creates `.git-fs/` and writes a project-local `.mcp.json`.
7. **Verify.** Restart the agent. Confirm the session banner shows `Branch: agent/<uuid>`.

## Manual install (no agent)

If you'd rather wire it up yourself:

```bash
# 1. download archive for your platform from /releases/latest
# 2. extract git-fs + git-fs-mcp to a PATH dir (e.g. ~/.local/bin/)
chmod +x ~/.local/bin/git-fs ~/.local/bin/git-fs-mcp

# 3. bootstrap a project
cd /path/to/project
git-fs init-project    # creates .git-fs/ and .mcp.json
```

Then register the MCP server and (optionally) hooks in your agent's config. The exact paths and JSON shape vary per agent — open the agent guide above for specifics.

## Adding a new agent

Add `agents/<agent-name>/install.md` and `agents/<agent-name>/update.md`. Use the Claude Code files as templates. Open a PR.
