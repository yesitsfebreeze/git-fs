#!/usr/bin/env node
// git-fs plugin launcher (TypeScript port edition).
// Responsibilities:
//   1. Auto-init project on first session-start: bare repo + .mcp.json + settings.json deny rules.
//   2. Dispatch to bundled TS entry points:
//        node launcher.js mcp                    -> exec dist/mcp.js
//        node launcher.js hook session-start     -> exec dist/cli.js hook session-start
//        node launcher.js hook stop              -> exec dist/cli.js hook stop
//        node launcher.js hook read              -> exec dist/cli.js hook read
//        node launcher.js hook post-write        -> exec dist/cli.js hook post-write
//        node launcher.js hook post-edit         -> exec dist/cli.js hook post-edit
//
// Pure node — no external deps.

import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import process from 'node:process';

const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT
	|| resolve(dirname(fileURLToPath(import.meta.url)), '..');
const DIST = join(PLUGIN_ROOT, 'dist');
const CLI = join(DIST, 'cli.js');
const MCP = join(DIST, 'mcp.js');

function log(msg) {
	process.stderr.write(`[git-fs plugin] ${msg}\n`);
}

function isGitRepo(cwd) {
	return existsSync(join(cwd, '.git'));
}

function autoInitProject(cwd) {
	if (!isGitRepo(cwd)) return;
	if (existsSync(join(cwd, '.git-fs'))) return;
	log(`auto-initializing git-fs in ${cwd}`);
	const r = spawnSync(process.execPath, [CLI, 'init-project'], { cwd, stdio: 'inherit' });
	if (r.status !== 0) {
		log(`git-fs init-project failed (status ${r.status}); skipping`);
		return;
	}
	ensureSettingsDeny(cwd);
}

function ensureSettingsDeny(cwd) {
	const settingsPath = join(cwd, '.claude', 'settings.json');
	let settings = {};
	if (existsSync(settingsPath)) {
		try { settings = JSON.parse(readFileSync(settingsPath, 'utf8')); }
		catch (e) { log(`cannot parse ${settingsPath} (${e.message}); leaving untouched`); return; }
	} else {
		mkdirSync(dirname(settingsPath), { recursive: true });
	}
	settings.permissions = settings.permissions || {};
	const deny = new Set(settings.permissions.deny || []);
	const before = deny.size;
	deny.add('Edit');
	deny.add('Write');
	if (deny.size === before) return;
	settings.permissions.deny = [...deny];
	writeFileSync(settingsPath, JSON.stringify(settings, null, 2) + '\n');
	log(`updated ${settingsPath}: deny Edit/Write (forces edits through git-fs MCP)`);
}

function execNode(entry, args) {
	const child = spawn(process.execPath, [entry, ...args], {
		stdio: 'inherit',
		env: process.env,
	});
	child.on('exit', (code, signal) => {
		if (signal) process.kill(process.pid, signal);
		else process.exit(code ?? 0);
	});
	child.on('error', (err) => {
		log(`failed to spawn ${entry}: ${err.message}`);
		process.exit(1);
	});
}

function main() {
	const argv = process.argv.slice(2);
	const mode = argv[0];

	if (!mode) {
		log('usage: launcher.js <mcp|hook (session-start|stop|read|post-write|post-edit)>');
		process.exit(2);
	}

	if (mode === 'mcp') {
		execNode(MCP, []);
		return;
	}

	if (mode === 'hook') {
		const sub = argv[1];
		if (!sub) { log('hook needs subcommand'); process.exit(2); }
		const cwd = process.cwd();
		if (sub === 'session-start') autoInitProject(cwd);
		execNode(CLI, ['hook', sub]);
		return;
	}

	log(`unknown mode: ${mode}`);
	process.exit(2);
}

main();
