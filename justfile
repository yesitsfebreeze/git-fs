# git-fs: Rust, one binary. `just build` is what the plugin needs.

# Release build into bin/git-fs (the path plugin.json and hooks.json call).
build:
	env -u CARGO_TARGET_DIR cargo build --release --locked
	mkdir -p bin
	cp target/release/git-fs bin/git-fs.new && mv bin/git-fs.new bin/git-fs

# Debug build into bin/, for quick iteration.
build-debug:
	env -u CARGO_TARGET_DIR cargo build --locked
	mkdir -p bin
	cp target/debug/git-fs bin/git-fs.new && mv bin/git-fs.new bin/git-fs

test:
	env -u CARGO_TARGET_DIR cargo test --locked

check:
	env -u CARGO_TARGET_DIR cargo fmt --all -- --check
	env -u CARGO_TARGET_DIR cargo clippy --all-targets --locked -- -D warnings
	env -u CARGO_TARGET_DIR cargo test --locked

# Register this checkout as a marketplace and install the plugin into the
# current Claude account ($CLAUDE_CONFIG_DIR).
install: build
	claude plugin marketplace add {{justfile_directory()}} 2>/dev/null || claude plugin marketplace update git-fs
	claude plugin install git-fs@git-fs --scope user
