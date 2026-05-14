# git-fs (TypeScript source)

This directory contains the TypeScript implementation of the `git-fs`
plugin. See the [repo README](../README.md) for project overview, install
instructions, and design rationale.

## Layout

```
src/
  store.ts   git object operations (isomorphic-git)
  mcp.ts     stdio JSON-RPC MCP server
  cli.ts     CLI subcommands + hook dispatch
  hooks.ts   SessionStart / PostToolUse / PreToolUse / Stop handlers
test/        node:test unit tests
bench/       perf benchmarks (in-process + MCP-driven)
build.mjs    esbuild bundler (emits ./dist and ../dist)
```

## Dev loop

```
npm install
npm run typecheck
npm test
npm run build       # writes ./dist and ../dist (plugin consumes the latter)
npm run bench       # 500-commit synthetic session
```

## Stack

- [isomorphic-git](https://isomorphic-git.org/) — pure-JS git core
- [node-diff3](https://github.com/bhousel/node-diff3) — 3-way line merge
- [diff](https://github.com/kpdecker/jsdiff) — unified-diff hunks
- [picomatch](https://github.com/micromatch/picomatch) — mergeignore globs
- [proper-lockfile](https://github.com/moxystudio/node-proper-lockfile) — cross-platform merge lock
- [esbuild](https://esbuild.github.io/) — single-file bundling
