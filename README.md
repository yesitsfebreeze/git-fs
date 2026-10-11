<div align="center">

# 🗂️ git-fs

**Git session branches for AI agents.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-server-7C3AED)](https://modelcontextprotocol.io)
[![Claude Code](https://img.shields.io/badge/Claude_Code-D97757)](#install)
[![Codex](https://img.shields.io/badge/Codex-000000)](#install)
[![Stars](https://img.shields.io/github/stars/yesitsfebreeze/git-fs?style=social)](https://github.com/yesitsfebreeze/git-fs/stargazers)

</div>

An agent's edits belong on its own branch. git-fs gives Claude Code and Codex a
Git-backed file layer: every edit is a commit, reads see the agent's changes,
and only changed files are copied back to your working directory.

- **Keep changes separate.** Each session works on its own branch.
- **Prevent stale edits.** Writes check that the file still matches what the agent read.
- **Review and undo.** Inspect a session's diff or roll back an edit.
- **Read in parallel.** Batch independent file reads and searches in one call.

## Install

Give your agent [install.txt](install.txt). It explains how to build git-fs,
connect any stdio MCP host, and use it without plugins or host-specific hooks.

Requires [Rust](https://rustup.rs), Git and
[ripgrep](https://github.com/BurntSushi/ripgrep).

```sh
git clone https://github.com/yesitsfebreeze/git-fs.git
cd git-fs
cargo install --path . --locked
```

Follow [install.txt](install.txt) to configure the MCP connection and persistent
agent instructions. Host-specific configuration is kept locally and ignored by Git.

## Use it

Once configured, the agent uses git-fs for reading, searching and editing files. Try:

> Refactor this module, then show me the git-fs diff.

> Read these files in a batch and explain how they fit together.

> Roll back the last git-fs edit.

Edits stay on the session branch until **materialize** copies them to disk.
The agent must materialize before builds and tests and before finishing work.
Existing disk changes are checked before anything is overwritten.

Materializing updates files, while **integrate** commits the reviewed session
changes onto your working branch. **Push** publishes them to your remote.

For tool arguments, batching examples, configuration and hook behavior, see the
[reference](docs/reference.md).

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=yesitsfebreeze/git-fs&type=Date)](https://star-history.com/#yesitsfebreeze/git-fs&Date)

## License

[MIT](LICENSE)
