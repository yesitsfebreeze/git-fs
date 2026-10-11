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

Give your agent [install.txt](install.txt). It explains how to install git-fs,
connect any stdio MCP host, and use it without plugins or host-specific hooks.

Download the ZIP for your platform from [Releases](https://github.com/yesitsfebreeze/git-fs/releases):
macOS or Linux, Intel/AMD or ARM64. Verify it against the release's
`SHA256SUMS`, extract it, then install the binary for use from any project:

```sh
mkdir -p "$HOME/.local/bin"
install -m 755 git-fs "$HOME/.local/bin/git-fs"
export PATH="$HOME/.local/bin:$PATH"
git-fs tools
```

Requires Git and [ripgrep](https://github.com/BurntSushi/ripgrep) at runtime.
No Rust compiler is needed. Linux downloads require glibc 2.35 or newer.
See [install.txt](install.txt) for platform selection, checksum verification,
persistent PATH setup, source builds, and MCP configuration. Until the first
binary release is published, use its source-build instructions.
Host-specific configuration is kept locally and ignored by Git.

## Binary releases

[Build binaries](.github/workflows/build.yml) tests and builds all four platforms
on pushes to `main`, pull requests, and manual runs. Each successful build
uploads a ZIP and SHA-256 checksum as a workflow artifact (retained for 14 days).
Each ZIP contains the executable, license, and `install.txt`.

To publish, set the package version in `Cargo.toml`, update `Cargo.lock`, and
push a matching tag such as `v4.3.0`. The workflow rejects mismatched tags and
publishes a GitHub Release with all four ZIPs and `SHA256SUMS` only after every
platform passes. Release downloads persist beyond workflow artifact retention.

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
