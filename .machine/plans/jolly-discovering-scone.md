# Plan: Bump version and push

## Context
The user wants to cut a new release of `git-fs`. Current version is `3.0.3` (last commit `b227242 Release 3.0.3`). Version is tracked in two files that must stay in sync. There is also an uncommitted working-tree change to `.claude/settings.json` (removes a `permissions` block that denied `Edit`/`Write` and set `bypassPermissions`).

## Changes
1. Bump version `3.0.3` → `3.0.4` in:
   - `package.json` (line 3)
   - `.claude-plugin/plugin.json` (line 4)
2. Commit the version bump.
3. Push to `origin/main`.

## Open decision
`.claude/settings.json` is modified in the working tree (unrelated permissions change). Decide whether to include it in the release commit or leave it uncommitted. See question to user.

## Commit message
Follow existing convention `Release 3.0.4: <summary>`, ending with the `Co-Authored-By` trailer.

## Verification
- `git show --stat HEAD` confirms only intended files changed and both version strings read `3.0.4`.
- `git push` succeeds; `git status` clean (or only settings.json remaining if excluded).
