# Status line never wired by /oil — statusbar absent after install + /oil + restart

- **date:** 2026-06-16
- **severity:** major
- **area:** skill (oil) / drill bring-up (assemble.md) / status line config
- **status:** fixed (machine source: /home/feb/dev/machine, uncommitted)

## What happened
Installed the machine plugin in a repo, ran `/oil`, restarted Claude Code — no
statusbar appeared. `/oil`'s SKILL.md explicitly delegates status line wiring to
drill's bring-up (`references/assemble.md`), so running `/oil` alone never writes the
`statusLine` block. The project's `.claude/settings.json` did contain a `statusLine`,
but its command was the relative `node .claude/hooks/statusline.mjs` pointing at a
file that does not exist in the project — the script lives only in the versioned
plugin cache at `${CLAUDE_PLUGIN_ROOT}/.claude/hooks/statusline.mjs`. Result: the
status line command silently fails and nothing renders.

## Expected
After installing the machine and running the documented post-install entry point
(`/oil`), a restart should show the machine statusbar. `/oil` is described as "run
after installing the machine plugin in a new repo," so a user reasonably expects it
to leave a working statusbar — but the wiring lives in a different code path
(drill bring-up) the user may never trigger.

## Evidence
```
$ ls /home/feb/dev/git-fs/.claude/hooks/statusline.mjs
"/home/feb/dev/git-fs/.claude/hooks/statusline.mjs": No such file or directory

# settings.json had:
"statusLine": { "type": "command", "command": "node .claude/hooks/statusline.mjs", "refreshInterval": 10 }

# script only exists (and works) in the plugin cache:
$ echo '{"hook_event_name":"statusLine","workspace":{"current_dir":"/home/feb/dev/git-fs"},"model":{"display_name":"Sonnet"}}' \
    | node /home/feb/.claude/plugins/cache/machine/machine/0.5.0/.claude/hooks/statusline.mjs
~/dev/git-fs │ git-fs │ ⎇ main*10 │ Sonnet
```

## Context
- repo: /home/feb/dev/git-fs (machine plugin installed via /plugin), OS Linux, branch main, driver role.
- oil SKILL.md:67 — "Status line and API keys are wired by drill's bring-up (references/assemble.md), not here."
- assemble.md:131-157 — the only place that writes the `statusLine` block, and it correctly
  resolves `${CLAUDE_PLUGIN_ROOT}/.claude/hooks/statusline.mjs` to an absolute literal.

## Verified capability (official docs, 2026-06-16)
- A plugin **cannot** contribute the main `statusLine`. Per
  https://code.claude.com/docs/en/plugins.md a plugin's bundled `settings.json` honors
  **only** the `agent` and `subagentStatusLine` keys; `statusLine` is read solely from
  user/project/local settings. The machine plugin's bundled `statusLine` block
  (`.claude/settings.json` in the plugin) is therefore **ignored** — dead config.
- There is **no** plugin.json / manifest / hooks.json field to ship a main status
  line script. `${CLAUDE_PLUGIN_ROOT}` is documented to expand in skill/agent/hook/MCP
  command strings, but NOT documented for `statusLine.command`, and it does NOT expand
  inside a project `settings.json`.
- Conclusion: wiring the status line into the target repo's `.claude/settings.json` is
  unavoidable. The user's hoped-for "just set it in the plugin" is not currently
  possible in Claude Code.

## Suspected cause / fix
Two coupled defects:
1. **Wiring gap.** `/oil` is the advertised post-install action but deliberately does
   NOT wire the status line (delegated to drill bring-up). Fix: have `/oil` perform the
   wiring itself — idempotent, owning only the `statusLine` key — so the documented
   entry point leaves a working statusbar.
2. **Path rot on update.** Since wiring must land in the project, avoid baking the
   versioned cache dir (`.../machine/0.5.0/...`) which a `/plugin update` invalidates.
   Robust options (pick one for the machine source):
   - **A — re-resolve on every `/oil` run.** Overwrite `statusLine.command` with the
     freshly-resolved `${CLAUDE_PLUGIN_ROOT}` path each run; self-heals, but stays
     broken between an update and the next `/oil`.
   - **B — stable user-level shim.** `bootstrap.sh` installs/refreshes the script to a
     non-versioned path (e.g. `~/.claude/hooks/machine-statusline.mjs`) on every
     install/update; projects point at that stable path once and never rot.
   - **C — vendor into the repo.** Copy `statusline.mjs` into the project's
     `.claude/hooks/` and use the relative path (makes the original relative command
     correct); self-contained but script updates don't propagate and it crosses the
     "no plugin content in the project" boundary.

## Resolution (chosen: option B — stable user-level shim)
Implemented in the machine source repo (`/home/feb/dev/machine`):
- **`scripts/bootstrap.sh`** — new `ensure_statusline_shim()` copies
  `$REPO_ROOT/.claude/hooks/statusline.mjs` → `~/.claude/hooks/machine-statusline.mjs`
  on every run (added to the run sequence). Version-independent; a `/plugin update`
  refreshes the shim automatically.
- **`assemble.md`** (drill bring-up) — status-line wiring now writes
  `node ~/.claude/hooks/machine-statusline.mjs` into the repo's settings, no longer the
  versioned cache path; doc corrected with the verified plugin limitation.
- **`oil/SKILL.md`** — added a self-heal step: `/oil` now ensures the shim exists and
  wires the repo's `statusLine` key, so install + `/oil` + restart yields a working
  statusbar without depending on drill bring-up. Boundary updated to permit the single
  `statusLine` key in the target repo + the `~/.claude/hooks/` shim.

Immediate remediation in THIS repo (git-fs): shim installed at
`~/.claude/hooks/machine-statusline.mjs`; `.claude/settings.json` `statusLine.command`
set to `node ~/.claude/hooks/machine-statusline.mjs`. Verified renders. Restart to show.

Follow-up: commit the machine-source changes and `/plugin update machine` so other repos
pick them up. (Pending — source changes are uncommitted.)
