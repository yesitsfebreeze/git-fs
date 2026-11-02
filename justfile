# git-fs: Rust, one binary. `just build` is what the plugin needs.
#
# The build tree lives outside the checkout: `claude plugin install` copies
# the whole plugin directory into its cache, and a target/ in it would be
# copied too (1.4 GB measured). Override with CARGO_TARGET_DIR.

target := env("CARGO_TARGET_DIR", env("HOME") + "/.cache/git-fs/target")
plugin_root := env("GIT_FS_PLUGIN_ROOT", justfile_directory() + "/../git-fs-plugin")

export CARGO_TARGET_DIR := target

# Release build into bin/git-fs (the path plugin.json and hooks.json call).
# Rename a running binary aside first: Windows refuses to replace an image
# another process (this session's MCP server) has open.
build:
	cargo build --release --locked
	mkdir -p bin
	cp {{quote(target)}}/release/git-fs bin/git-fs.new
	@if [ -f bin/git-fs ]; then mv bin/git-fs bin/git-fs.old-$(date +%s); fi
	mv bin/git-fs.new bin/git-fs
	@rm -f bin/git-fs.old-* 2>/dev/null || true

# Debug build into bin/, for quick iteration.
build-debug:
	cargo build --locked
	mkdir -p bin
	cp {{quote(target)}}/debug/git-fs bin/git-fs.new
	@if [ -f bin/git-fs ]; then mv bin/git-fs bin/git-fs.old-$(date +%s); fi
	mv bin/git-fs.new bin/git-fs
	@rm -f bin/git-fs.old-* 2>/dev/null || true

# Copy the built binary into the directory marketplace the host actually
# loads, with the same rename-aside swap. Takes effect at the next session
# start or /reload-plugins.
install-plugin: build
	mkdir -p {{quote(plugin_root)}}/bin
	cp bin/git-fs {{quote(plugin_root)}}/bin/git-fs.new
	@if [ -f {{quote(plugin_root)}}/bin/git-fs ]; then mv {{quote(plugin_root)}}/bin/git-fs {{quote(plugin_root)}}/bin/git-fs.old-$(date +%s); fi
	mv {{quote(plugin_root)}}/bin/git-fs.new {{quote(plugin_root)}}/bin/git-fs
	@rm -f {{quote(plugin_root)}}/bin/git-fs.old-* 2>/dev/null || true

test:
	cargo test --locked

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --locked -- -D warnings
	cargo test --locked

# Build, register this checkout as marketplace `git-fs` and install the
# plugin in the current Claude account ($CLAUDE_CONFIG_DIR). A directory
# marketplace runs the plugin from this folder: a new binary takes effect at
# the next session start or /reload-plugins.
install: build
	claude plugin marketplace add {{justfile_directory()}} 2>/dev/null || claude plugin marketplace update git-fs
	claude plugin install git-fs@git-fs --scope user 2>/dev/null || claude plugin update git-fs@git-fs

# Build and install the Codex adapter from this local marketplace.
install-codex: build
	codex plugin marketplace add {{quote(justfile_directory())}}
	codex plugin add git-fs@git-fs
