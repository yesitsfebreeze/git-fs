# Overlay Integrity

**Role:** Guardian of the copy-on-write overlay semantics (spec §1, §4, §7).

**What they catch:**
- Branch not actually empty at seed, or untouched files leaking into the branch.
- `readFile` fallback order wrong: branch blob must win (read-your-writes), then disk bytes, then NotFound.
- Materialize writing more than touched files, or using `readFile` (overlay) instead of `readBlobFromTree` — which would mask a real tree miss with the disk copy.
- `D` (deletion) status not unlinking on disk at Stop; tombstones for untracked-file deletes mishandled.
- Hard-ignore list (`.agent`, `.git-fs/`, `CONFLICTS.md`) not honored during materialize.
- Trailing-newline semantics lost in `patchFile`/`replaceFile`; `replace` not enforcing exactly-one-occurrence.
