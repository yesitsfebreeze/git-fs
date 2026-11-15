<div align="center">

# 🗂️ git-fs

**Live Git commits for collaborating AI agents.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-server-7C3AED)](https://modelcontextprotocol.io)
[![Stars](https://img.shields.io/github/stars/yesitsfebreeze/git-fs?style=social)](https://github.com/yesitsfebreeze/git-fs/stargazers)

</div>

git-fs gives MCP-capable agents a shared, Git-backed file view. Every successful
edit or deletion becomes a durable commit on `refs/gitfs-live/current`, visible
immediately to other sessions on the same repository—without touching the
working tree, index, or reviewed `main` branch.

- **Collaborate live.** Reads and searches see the shared commit; independent edits merge, conflicting edits refuse without overwriting a peer.
- **Keep ownership.** Session branches still record provenance, rollback, and review history.
- **Build independently.** `gitfs_snapshot` creates a detached worktree pinned to one exact live commit.
- **Review before release.** Integration and remote push remain separate, guarded operations.
- **Batch work.** Every tool accepts one item or an `items` array; read/search items run concurrently.

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
Guarded push requires Git 2.40 or newer to run existing pre-push hooks.
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
push a matching tag such as `v4.3.1`. The workflow rejects mismatched tags and
publishes a GitHub Release with all four ZIPs and `SHA256SUMS` only after every
platform passes. Release downloads persist beyond workflow artifact retention.

## Use it

Once configured, the agent uses git-fs for reading, searching and editing files. Try:

> Refactor this module, then show me the git-fs diff.

> Read these files in a batch and explain how they fit together.

> Roll back the last git-fs edit.

Changes publish immediately to the shared live Git ref, but the ordinary disk
checkout remains unchanged. Use `gitfs_snapshot {}` to get a clean pinned
worktree and run independent builds/tests there; pass its returned `id` to
`gitfs_snapshot` to release it after the checkout is clean. `gitfs_materialize`
remains available when a native tool must work in the common checkout and checks
for disk conflicts. The session branch remains the review and rollback record.

Shared live commits are **local to that Git repository**. They are not
automatically pushed to GitHub: `integrate` and the guarded `push` workflow
still control reviewed publication to `main`. To share between machines, use
the reviewed Git remote handoff.

For tool arguments, batching examples, configuration and hook behavior, see the
[reference](docs/reference.md).

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=yesitsfebreeze/git-fs&type=Date)](https://star-history.com/#yesitsfebreeze/git-fs&Date)

## License

[MIT](LICENSE)
