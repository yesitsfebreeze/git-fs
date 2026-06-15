# Git Plumbing

**Role:** Keeps the git object/ref/tree mechanics correct and the store isolated (spec §3, §5, §6).

**What they catch:**
- Any write to the project's own `.git` — D3 forbids it; the store is `.git-fs/` only.
- Tree mutations not using a unique scratch `GIT_INDEX_FILE` under `.git-fs/tmp/`, or not deleting it — corrupting the store's index.
- Missing per-branch `mkdir`-based lock under concurrent writers.
- `commit-tree` parent handling (empty seed has no parent; subsequent commits chain on tip).
- `update-ref` targeting the wrong ref namespace (`refs/heads/gitfs/<id>`).
- `merge-tree --write-tree` version assumptions (git ≥2.38) and conflict-marker parsing.
- `cat-file`/`ls-tree`/`diff` invoked without `--git-dir=<GITFS>`.
