#!/usr/bin/env node
// Overwatch for Claude Code: a thin translator between Claude Code's command hooks and the neutral
// hook contract of overwatch (`overwatch hook <event>`, in the overwatch-obsidian main branch). It
// decides nothing. Claude Code runs it as a command hook: the hook's input JSON arrives on stdin, it
// is turned into one neutral event and given to `overwatch hook <event>`, and the answer is printed
// in the form Claude Code reads.
//
//   node hook.mjs                              the hook: Claude Code's input on stdin, its output on stdout
//   node hook.mjs statusline                   the status line: Claude Code's statusLine input on stdin, one line on stdout
//   node hook.mjs install [--committed] [--statusline] [<project>]
//                                              installs this file and its hooks (asked, also the status line) into the project
//
// overwatch is found as OVERWATCH_BIN, or as `overwatch` on the PATH (npm link in an overwatch
// checkout puts it there). The neutral contract: stdin {"session", "cwd", "tool": {"name",
// "input"}, "prompt"}; stdout {"decision": "allow" | "block", "reason", "context", "updatedInput"}
// or nothing. Claude Code's tool names are the contract's tool names, so a tool passes as it is.
//
// It fails open: when overwatch is missing, prints nothing, does not answer in time or anything else
// goes wrong, it prints nothing and exits 0, which Claude Code reads as "no decision" (one line on
// stderr says why). A closed Obsidian never blocks or breaks an agent.
//
// What it relies on, from Claude Code's hooks reference (https://code.claude.com/docs/en/hooks, read
// 2026-11-17):
//  - input, every event: `hook_event_name`, `session_id` and `cwd` (the folder the agent works in);
//    PreToolUse: `tool_name`, `tool_input`; UserPromptSubmit: `prompt`. PostToolUse (a tool call
//    that succeeded): `tool_name`, `tool_input`, `tool_response` (any JSON value), `tool_use_id`.
//    PostToolUseFailure (a tool call that failed): `tool_name`, `tool_input`, `tool_use_id`, `error`
//    (the text Claude gets as the failed result). Both match on the tool name as PreToolUse does.
//  - exit code 0 with one JSON object on stdout: its fields apply; with nothing on stdout: no decision.
//  - Stop: `{ "decision": "block", "reason": "..." }` keeps the agent working, and the reason is
//    given to it.
//  - PreToolUse: `hookSpecificOutput` with `hookEventName: "PreToolUse"`; `permissionDecision:
//    "deny"` with `permissionDecisionReason` refuses the call and gives the agent the reason;
//    `updatedInput` replaces the whole input of the call. Without a `permissionDecision` the call
//    goes through the user's normal permission flow, so the translator never approves a call itself.
//  - SessionStart and UserPromptSubmit: `hookSpecificOutput` with `hookEventName` and
//    `additionalContext`, a text added to the agent's context.
//  - statusLine `{ type: "command", command }` (https://code.claude.com/docs/en/statusline): the
//    command gets a JSON object on stdin with `session_id`, `cwd` and `workspace.current_dir`, and
//    what it prints is the status line (ANSI colors and OSC 8 links shown). It asks overwatch the
//    neutral event `status` and prints the answer's `context`.
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** How long overwatch may take; Claude Code gives the hook 10 seconds (SETTINGS below). */
const TIMEOUT_MS = 8000;

/** Claude Code's hook events, by the neutral event each one is. */
export const EVENTS = { SessionStart: 'session-start', UserPromptSubmit: 'prompt', PreToolUse: 'before-tool', PostToolUse: 'after-tool', PostToolUseFailure: 'after-tool', Stop: 'stop' };

/**
 * A tool's result as text: a string as it is; Bash's `{ stdout, stderr }` as its output followed by
 * its error output; any other value as JSON. Undefined when Claude Code gave none.
 */
export function resultText(response) {
  if (response === undefined || response === null) return undefined;
  if (typeof response === 'string') return response;
  if (typeof response === 'object' && typeof response.stdout === 'string') return [response.stdout, typeof response.stderr === 'string' ? response.stderr : ''].filter(Boolean).join('\n');
  return JSON.stringify(response);
}

/** Claude Code's hook input as the neutral event. */
export function neutral(input) {
  const text = value => (typeof value === 'string' ? value : '');
  return {
    session: text(input.session_id),
    ...(text(input.cwd) ? { cwd: input.cwd } : {}),
    ...(input.hook_event_name === 'PreToolUse' ? { tool: { name: text(input.tool_name), input: input.tool_input && typeof input.tool_input === 'object' ? input.tool_input : {} } } : {}),
    ...(input.hook_event_name === 'UserPromptSubmit' && text(input.prompt) ? { prompt: input.prompt } : {}),
    // After a tool: success is the event Claude Code ran, the output its result or its error text.
    ...(input.hook_event_name === 'PostToolUse' || input.hook_event_name === 'PostToolUseFailure' ? { tool: afterTool(input) } : {}),
  };
}

function afterTool(input) {
  const failed = input.hook_event_name === 'PostToolUseFailure';
  const output = failed ? (typeof input.error === 'string' ? input.error : undefined) : resultText(input.tool_response);
  return {
    name: typeof input.tool_name === 'string' ? input.tool_name : '',
    input: input.tool_input && typeof input.tool_input === 'object' ? input.tool_input : {},
    ...(output === undefined ? {} : { output }),
    ok: !failed,
    ...(typeof input.tool_use_id === 'string' && input.tool_use_id ? { id: input.tool_use_id } : {}),
  };
}

/** overwatch's answer as Claude Code's hook output for `event`, or null when there is nothing to say. */
export function output(event, answer) {
  const blocked = answer.decision === 'block';
  if (event === 'Stop') return blocked ? { decision: 'block', reason: answer.reason } : null;
  const specific = event === 'PreToolUse'
    ? { ...(blocked ? { permissionDecision: 'deny', permissionDecisionReason: answer.reason } : {}), ...(answer.updatedInput ? { updatedInput: answer.updatedInput } : {}) }
    : answer.context ? { additionalContext: answer.context } : {};
  return Object.keys(specific).length ? { hookSpecificOutput: { hookEventName: event, ...specific } } : null;
}

/** What `overwatch hook <event>` prints for this neutral event: its answer, or '' when it decided nothing. */
function ask(event, input) {
  const bin = process.env.OVERWATCH_BIN || 'overwatch';
  return new Promise((done, fail) => {
    // Windows runs no `#!` script: a file named by OVERWATCH_BIN that is not a program (bin/overwatch is a Node script) runs with this Node.
    const script = process.platform === 'win32' && existsSync(bin) && !/\.(exe|com)$/i.test(bin);
    const child = spawn(script ? process.execPath : bin, [...(script ? [bin] : []), 'hook', event], { stdio: ['pipe', 'pipe', 'inherit'], timeout: TIMEOUT_MS });
    let out = '';
    child.stdout.on('data', chunk => { out += chunk; });
    child.on('error', error => fail(new Error(error.code === 'ENOENT' ? `${bin} is not on the PATH (set OVERWATCH_BIN, or npm link in an overwatch checkout)` : error.message)));
    child.on('close', (code, signal) => (signal ? fail(new Error(`${bin} hook ${event} took longer than ${TIMEOUT_MS / 1000} seconds`)) : done(out.trim())));
    child.stdin.end(JSON.stringify(input));
  });
}

/** Claude Code's status line input as the neutral event `status`: the session and the folder it works in. */
export function statusEvent(input) {
  const text = value => (typeof value === 'string' ? value : '');
  const cwd = text(input?.workspace?.current_dir) || text(input?.cwd);
  return { session: text(input?.session_id), ...(cwd ? { cwd } : {}) };
}

/** The status line: overwatch's `context` for the event `status`, or '' when it said nothing. */
async function statusline() {
  let text = '';
  for await (const chunk of process.stdin) text += chunk;
  const answer = await ask('status', statusEvent(JSON.parse(text)));
  const context = answer ? JSON.parse(answer).context : '';
  return typeof context === 'string' ? context : '';
}

async function hook() {
  let text = '';
  for await (const chunk of process.stdin) text += chunk;
  const input = JSON.parse(text);
  // An event overwatch does not know is no question for it.
  if (!Object.hasOwn(EVENTS, input.hook_event_name)) return '';
  const answer = await ask(EVENTS[input.hook_event_name], neutral(input));
  const out = answer ? output(input.hook_event_name, JSON.parse(answer)) : null;
  return out ? JSON.stringify(out) : '';
}

// --- install ---

/** Where the translator goes in a project. */
export const FORWARDER = '.claude/overwatch/hook.mjs';
/** The settings files that can register it: the local one is personal (Claude Code reads it only on this machine, usually git-ignored); the other is committed. */
export const LOCAL_SETTINGS = '.claude/settings.local.json';
export const COMMITTED_SETTINGS = '.claude/settings.json';
/** The vault's record of installed agents, which makes overwatch's enforcement available. */
export const RECORD = 'docs/vault/.overwatch/hooks.json';
const command = { type: 'command', command: `node "\${CLAUDE_PROJECT_DIR}/${FORWARDER}"`, timeout: 10 };
/** The status line it registers when asked (`--statusline`); without the variable, the folder the status line runs in. */
export const STATUS_LINE = { type: 'command', command: `node "\${CLAUDE_PROJECT_DIR:-.}/${FORWARDER}" statusline` };
/** The tools the hooks before and after a tool run for: the editing tools, Bash and every MCP tool. */
const TOOLS = '^(Edit|Write|MultiEdit|NotebookEdit|Bash|mcp__.*)$';
/** The hooks it registers: session start, prompt, stop, and before and after (succeeded or failed) the editing tools, Bash and every MCP tool. */
export const SETTINGS = {
  hooks: {
    SessionStart: [{ hooks: [command] }],
    UserPromptSubmit: [{ hooks: [command] }],
    PreToolUse: [{ matcher: TOOLS, hooks: [command] }],
    PostToolUse: [{ matcher: TOOLS, hooks: [command] }],
    PostToolUseFailure: [{ matcher: TOOLS, hooks: [command] }],
    Stop: [{ hooks: [command] }],
  },
};

const isObject = value => !!value && typeof value === 'object' && !Array.isArray(value);
/** The JSON object in the file; `{}` for a missing file. A file that holds anything else is an error, never silently replaced. */
function objectIn(file, name) {
  if (!existsSync(file)) return {};
  let parsed;
  try { parsed = JSON.parse(readFileSync(file, 'utf8')); } catch (error) { throw new Error(`${name} is not valid JSON: ${error.message}`); }
  if (!isObject(parsed)) throw new Error(`${name} must hold a JSON object`);
  return parsed;
}
const write = (file, text) => { mkdirSync(dirname(file), { recursive: true }); writeFileSync(file, text); };
/** Whether a hook entry of a settings file runs this translator: its command names its path. */
const isForwarder = hook => isObject(hook) && typeof hook.command === 'string' && hook.command.includes(FORWARDER);
const groupHooks = group => (isObject(group) && Array.isArray(group.hooks) ? group.hooks : []);
/** An event's hook groups with the translator's own entries taken out: every other hook stays where it is, and a group that held only the translator goes. */
const withoutForwarder = groups => groups.flatMap(group => {
  if (!groupHooks(group).some(isForwarder)) return [group];
  const rest = groupHooks(group).filter(hook => !isForwarder(hook));
  return rest.length ? [{ ...group, hooks: rest }] : [];
});

/**
 * Installs the translator into the project that holds the vault: writes `.claude/overwatch/hook.mjs`,
 * puts SETTINGS's hooks into the local settings, or into the committed `.claude/settings.json` with
 * `committed`, takes its entries out of the other settings file (Claude Code runs both), and records
 * the installation in the vault's `.overwatch/hooks.json`. Every other key
 * and every other hook of the settings stays; the translator's own entries (found by the path in
 * their command) are replaced by SETTINGS's where they differ, so a reinstall brings an outdated
 * entry up to date. With `statusline`, the settings' `statusLine` runs the translator too; one that
 * runs another command is refused, never replaced. Returns what it wrote; nothing when everything was in place.
 */
export function install(project, { committed = false, statusline = false, hook = readFileSync(fileURLToPath(import.meta.url), 'utf8'), now = Date.now() } = {}) {
  if (!existsSync(join(project, 'docs', 'vault'))) throw new Error(`${project} has no docs/vault; run this in the project that holds the vault`);
  const target = committed ? COMMITTED_SETTINGS : LOCAL_SETTINGS, settings = objectIn(join(project, target), target);
  if (settings.hooks !== undefined && !isObject(settings.hooks)) throw new Error(`"hooks" in ${target} must be an object`);
  const ours = isObject(settings.statusLine) && typeof settings.statusLine.command === 'string' && settings.statusLine.command.includes(FORWARDER);
  if (statusline && settings.statusLine !== undefined && !ours) throw new Error(`${target} has a statusLine of its own; to show overwatch's line in it, have its command also run: ${STATUS_LINE.command}`);
  const status = statusline && JSON.stringify(settings.statusLine) !== JSON.stringify(STATUS_LINE);
  const hooks = { ...settings.hooks };
  const added = [], updated = [];
  for (const [event, groups] of Object.entries(SETTINGS.hooks)) {
    const there = hooks[event] ?? [];
    if (!Array.isArray(there)) throw new Error(`"hooks.${event}" in ${target} must be a list`);
    const others = withoutForwarder(there), text = group => JSON.stringify(group);
    // In place already: the groups as they are, and the translator in no other entry.
    if (others.length + groups.length === there.length && groups.every(group => there.some(found => text(found) === text(group)))) continue;
    hooks[event] = [...others, ...groups];
    (there.some(group => groupHooks(group).some(isForwarder)) ? updated : added).push(event);
  }

  const wrote = [];
  const forwarder = join(project, FORWARDER);
  if (!existsSync(forwarder) || readFileSync(forwarder, 'utf8') !== hook) { write(forwarder, hook); wrote.push(FORWARDER); }
  if (added.length || updated.length || status) {
    write(join(project, target), JSON.stringify({ ...settings, hooks, ...(statusline ? { statusLine: STATUS_LINE } : {}) }, null, 2) + '\n');
    wrote.push(`${target}, ${committed ? 'committed: every collaborator and CI run of Claude Code here runs the translator' : 'only yours'} (${[added.length ? `hooks added: ${added.join(', ')}` : '', updated.length ? `hooks updated: ${updated.join(', ')}` : '', status ? 'status line set' : ''].filter(Boolean).join('; ')})`);
  }
  // Claude Code runs the hooks of both settings files, so the one not chosen here must not keep a copy:
  // an entry left in it would post every event twice.
  const other = committed ? LOCAL_SETTINGS : COMMITTED_SETTINGS, elsewhere = objectIn(join(project, other), other);
  const otherHooks = isObject(elsewhere.hooks) ? elsewhere.hooks : undefined, dropped = [];
  for (const [event, groups] of Object.entries(otherHooks ?? {})) {
    if (!Array.isArray(groups)) continue;
    const rest = withoutForwarder(groups);
    if (rest.length === groups.length) continue;
    dropped.push(event);
    if (rest.length) otherHooks[event] = rest; else delete otherHooks[event];
  }
  if (dropped.length) {
    if (!Object.keys(otherHooks).length) delete elsewhere.hooks;
    write(join(project, other), JSON.stringify(elsewhere, null, 2) + '\n');
    wrote.push(`${other}, the translator's entries removed (${dropped.join(', ')}): Claude Code runs the hooks of both settings files`);
  }
  const record = join(project, RECORD), installed = objectIn(record, RECORD), entry = installed['claude-code'];
  if (!isObject(entry) || entry.path !== FORWARDER) {
    write(record, JSON.stringify({ ...installed, 'claude-code': { installedAt: new Date(now).toISOString(), path: FORWARDER } }, null, 2) + '\n');
    wrote.push(RECORD);
  }
  return wrote;
}

// --- the command line ---

const [mode, ...rest] = process.argv.slice(2);
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (mode === 'install') {
    const flags = ['--committed', '--statusline'], project = resolve(rest.find(arg => !flags.includes(arg)) ?? '.');
    try {
      const wrote = install(project, { committed: rest.includes('--committed'), statusline: rest.includes('--statusline') });
      console.log(wrote.length ? `overwatch for Claude Code is installed in ${project}: ${wrote.join(', ')}. It applies to Claude Code sessions started from now on.` : `overwatch for Claude Code is already installed in ${project}; nothing changed.`);
    } catch (error) { console.error(`overwatch for Claude Code was not installed: ${error.message}`); process.exit(1); }
  } else if (mode === 'statusline') {
    // A status line that fails shows nothing; the reason goes to stderr.
    statusline().then(
      out => process.stdout.write(out, () => process.exit(0)),
      error => { console.error(`overwatch statusline (claude-code): ${error?.message ?? error}`); process.exit(0); },
    );
  } else {
    hook().then(
      out => process.stdout.write(out, () => process.exit(0)),
      error => { console.error(`overwatch hook (claude-code): ${error?.message ?? error}; nothing was decided`); process.exit(0); },
    );
  }
}
