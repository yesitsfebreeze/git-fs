# agent.md — git-fs project layer

## What this project is
git-fs presents each Claude Code session with a **copy-on-write overlay over the working directory**. The store is a dedicated bare git repo at `.git-fs/` (gitignored, publishable). A session works on branch `gitfs/<session-id>` that **starts empty**; a file enters the branch only when an agent writes/edits it. Reads fall back to the working tree, but the fallback is gated to paths the main git already tracks — ignored/untracked content (secrets, deps, build junk) stays out of the store unless re-permitted by a `.gitfsallow` file at the repo root (gitignore-style dir/suffix/exact patterns). Stop materializes only touched files back to disk. Net effect: `.git-fs` holds **pure deltas**.

## Project law (binding — as hard as machine law)
- **D1 — No build step.** Plain ESM `.mjs`, run directly by Node ≥20. No transpile, no bundler, no generated output dir. `bin` points straight at a `.mjs`.
- **D2 — Zero runtime npm dependencies.** All object/ref/tree/merge ops shell out to the system `git` binary via `node:child_process`. The stdio JSON-RPC loop is hand-rolled.
- **D3 — Dedicated store.** The store is a bare `.git-fs/`. The project's own `.git` is **read-only** to git-fs — never write refs/objects into it.
- **D4 — Overlay model.** Empty seed → disk-fallback reads → touched-only deltas → Stop materializes.
- **Graceful degrade.** If `git --version` fails, every hook and tool is a **no-op** that logs once to stderr; sessions still start. git-fs never hard-fails a session because it is unavailable (spec §3).
- **Scratch index discipline (spec §6).** Every tree mutation uses a unique temp `GIT_INDEX_FILE` under `.git-fs/tmp/`, then deletes it. Never disturb the store's own index. Serialize per-branch writers with a `mkdir`-based lock.
- **Materialize uses `readBlobFromTree`, not `readFile`** (spec §7) — the overlay fallback would mask a real tree miss with the disk copy.
- **Hard ignore** (never materialized): `.agent`, `.git-fs/`, `CONFLICTS.md` (spec §8).
- **The spec is normative.** Follow the §5/§7 pseudo code to the T; honor the §12 acceptance checklist as the definition of done.

## Domain idioms
- Tree edits go through `hash-object -w` → scratch-index `read-tree`/`update-index`/`write-tree` → `commit-tree` → `update-ref`. Never `git add`/`git commit` against a real worktree.
- `readFile(ref, path)`: branch blob wins (read-your-writes), else disk bytes, else NotFound.
- Merges use `git merge-tree --write-tree` (git ≥2.38); parse conflict markers on failure. `merge()` returns a *dangling* commit (no ref moved); `mergeLand(target, theirs)` lands it via a bounded recompute+CAS loop — re-read the target tip, 3-way merge, `commit-tree`, then `update-ref` with the old tip as the CAS guard; recompute on contention. CAS landings stay inside the `.git-fs` store (D3: never the project's real `.git`).
- `trailer(msg, branch)` appends `Session-Id: <id>` for `gitfs/*` branches; idempotent.

## Persona panel
See `.machine/personas.md`. Roster: **Overlay Integrity** (correctness of seed/fallback/materialize), **Git Plumbing** (scratch-index, refs, store isolation), **Zero-Dep Purist** (D1/D2 guard), **Resilience** (degrade-to-noop, locking, concurrency).

## Build / verify
- Test: `npm test`  (= `node --test`; no build, no `node_modules`). Under Node ≥24 avoid `node --test test/` (bare-dir arg → `MODULE_NOT_FOUND`).
- MCP: `node bin/git-fs.mjs mcp`   ·   Hook: `node bin/git-fs.mjs hook <action>`
- Acceptance: spec §12 binary checklist.
