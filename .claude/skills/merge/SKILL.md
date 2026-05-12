# Merge Skill

Merge current agent branch into `main`. Call when a body of work is complete.

## Trigger

User says: "merge", "merge to main", "commit this", "push to main", `/merge`

## Steps

1. Read current agent branch from session context (injected at SessionStart as `Branch: agent/...`)
2. Ensure `main` branch exists — if not, create it from current agent branch
3. Call `git_fs_merge` with:
   - `ours`: current agent branch
   - `theirs`: `main`
   - `base`: last common ancestor (use `main` as base if main is empty/new)
   - `into`: `main`
   - `message`: short description of what was done
4. On success: report commit OID and what merged
5. On conflict: list conflicting files, tell user what each agent changed, ask how to resolve

## Notes

- Do NOT auto-merge after every write — only on explicit user request
- `base` = the commit on `main` that both branches share. If `main` was just created from this branch, use the agent branch itself as base.
- If `main` doesn't exist yet: use `git_fs_branch_create` with `from: agent/{session_id}`, then report "main created from your branch"
- Conflicts mean another agent touched the same file — show the diff, let user decide
