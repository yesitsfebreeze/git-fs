# git-fs: Rust, one binary. `just build` is what the plugin needs.
#
# The build tree lives outside the checkout: `claude plugin install` copies
# the whole plugin directory into its cache, and a target/ in it would be
# copied too (1.4 GB measured). Override with CARGO_TARGET_DIR.

target := env("CARGO_TARGET_DIR", env("HOME") + "/.cache/git-fs/target")

export CARGO_TARGET_DIR := target

# Release build into bin/git-fs (the path plugin.json and hooks.json call).
build:
	cargo build --release --locked
	mkdir -p bin
	cp {{target}}/release/git-fs bin/git-fs.new && mv bin/git-fs.new bin/git-fs

# Debug build into bin/, for quick iteration.
build-debug:
	cargo build --locked
	mkdir -p bin
	cp {{target}}/debug/git-fs bin/git-fs.new && mv bin/git-fs.new bin/git-fs

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
