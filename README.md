# git-fs

A virtual filesystem over a bare git object store — no working tree required.

Each AI-agent session works on its own `agent/<session-id>` branch. Every Read / Write / Edit becomes a git commit. When the session ends, the agent branch is merged into `main` and materialized to disk.

**Why:** many agents can work on the same repo in parallel with no locking, no torn writes, and a full audit trail. Crash safety + history come for free.

## Install

Tell your agent to read the install guide and run it:

> Read https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/install.md and install git-fs.

The agent will:

1. Detect your platform.
2. Download the matching binary from the [latest release](https://github.com/yesitsfebreeze/git-fs/releases/latest).
3. Install the skill into `~/.claude/skills/git-fs/`.
4. Register the MCP server in `~/.claude/settings.json`.
5. Walk you through enabling the optional hooks that automate session-branching.

## Update

> Read https://github.com/yesitsfebreeze/git-fs/blob/main/agents/claude/update.md and update git-fs.

## Other agents

Currently only Claude Code is supported. PRs welcome under `agents/<agent>/`.

## Release artifacts

Every tagged release publishes precompiled binaries for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Plus a `SHA256SUMS` file. See https://github.com/yesitsfebreeze/git-fs/releases/latest.

## Architecture

- Bare git repo at `.git-fs/` is the source of truth.
- MCP server (`git-fs-mcp`) exposes file ops as MCP tools (`git_fs_read`, `git_fs_write`, `git_fs_replace`, etc.).
- Session hooks (`git-fs hook session-start | stop | read`) wire the agent into the lifecycle.
- Stop hook merges the agent branch into `main` with a mergeignore filter and an exclusive merge lock.

See [`docs/multi-agent-session.md`](docs/multi-agent-session.md) for the multi-agent design.

## Build from source

```bash
cargo build --release -p git-fs
```

Produces `target/release/git-fs` and `target/release/git-fs-mcp`.

## License

TBD.
