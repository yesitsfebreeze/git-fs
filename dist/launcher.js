#!/usr/bin/env node
// git-fs plugin launcher.
// Responsibilities:
//   1. On first call, download the matching `git-fs` + `git-fs-mcp` binaries
//      from the latest GitHub release and cache them under the plugin root.
//   2. Dispatch to the requested mode:
//        node launcher.js mcp                    -> exec git-fs-mcp (stdio inherited)
//        node launcher.js hook session-start     -> auto-init project, exec git-fs hook session-start
//        node launcher.js hook stop              -> exec git-fs hook stop
//        node launcher.js hook read              -> exec git-fs hook read
//
// Pure node — no external deps. Uses system `tar` (Unix) or `Expand-Archive` (Windows)
// for extraction.

import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, chmodSync, readFileSync, writeFileSync, renameSync, rmSync } from 'node:fs';
import { tmpdir, homedir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { request as httpsRequest } from 'node:https';
import { URL, fileURLToPath } from 'node:url';
import process from 'node:process';

const PLUGIN_ROOT = process.env.CLAUDE_PLUGIN_ROOT
	|| resolve(dirname(fileURLToPath(import.meta.url)), '..');
const BIN_DIR = join(PLUGIN_ROOT, 'bin');
const REPO = 'yesitsfebreeze/git-fs';

function log(msg) {
	process.stderr.write(`[git-fs plugin] ${msg}\n`);
}

function platformInfo() {
	const p = process.platform;
	const a = process.arch;
	if (p === 'linux' && a === 'x64') return { artifact: 'git-fs-x86_64-unknown-linux-gnu.tar.gz', kind: 'tgz', ext: '' };
	if (p === 'linux' && a === 'arm64') return { artifact: 'git-fs-aarch64-unknown-linux-gnu.tar.gz', kind: 'tgz', ext: '' };
	if (p === 'darwin' && a === 'x64') return { artifact: 'git-fs-x86_64-apple-darwin.tar.gz', kind: 'tgz', ext: '' };
	if (p === 'darwin' && a === 'arm64') return { artifact: 'git-fs-aarch64-apple-darwin.tar.gz', kind: 'tgz', ext: '' };
	if (p === 'win32' && a === 'x64') return { artifact: 'git-fs-x86_64-pc-windows-msvc.zip', kind: 'zip', ext: '.exe' };
	throw new Error(`unsupported platform: ${p}/${a}`);
}

function binaryPaths(info) {
	return {
		cli: join(BIN_DIR, `git-fs${info.ext}`),
		mcp: join(BIN_DIR, `git-fs-mcp${info.ext}`),
	};
}

function follow(url, depth = 0) {
	if (depth > 5) throw new Error('too many redirects');
	return new Promise((resolveP, rejectP) => {
		const u = new URL(url);
		const req = httpsRequest({
			hostname: u.hostname,
			path: u.pathname + u.search,
			method: 'GET',
			headers: { 'User-Agent': 'git-fs-plugin' },
		}, (res) => {
			if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
				res.resume();
				follow(new URL(res.headers.location, url).toString(), depth + 1).then(resolveP, rejectP);
				return;
			}
			if (res.statusCode !== 200) {
				rejectP(new Error(`HTTP ${res.statusCode} for ${url}`));
				return;
			}
			resolveP(res);
		});
		req.on('error', rejectP);
		req.end();
	});
}

async function downloadTo(url, dest) {
	const res = await follow(url);
	const { createWriteStream } = await import('node:fs');
	await new Promise((resolveP, rejectP) => {
		const out = createWriteStream(dest);
		res.pipe(out);
		out.on('finish', () => out.close(resolveP));
		out.on('error', rejectP);
		res.on('error', rejectP);
	});
}

function extract(archive, destDir, kind) {
	mkdirSync(destDir, { recursive: true });
	if (kind === 'tgz') {
		const r = spawnSync('tar', ['-xzf', archive, '-C', destDir], { stdio: 'inherit' });
		if (r.status !== 0) throw new Error('tar extraction failed');
	} else {
		const r = spawnSync('powershell.exe', [
			'-NoProfile', '-Command',
			`Expand-Archive -Force -Path '${archive}' -DestinationPath '${destDir}'`,
		], { stdio: 'inherit' });
		if (r.status !== 0) throw new Error('Expand-Archive failed');
	}
}

async function ensureBinaries() {
	const info = platformInfo();
	const { cli, mcp } = binaryPaths(info);
	if (existsSync(cli) && existsSync(mcp)) return { cli, mcp };

	log(`fetching git-fs binaries for ${process.platform}/${process.arch}...`);
	mkdirSync(BIN_DIR, { recursive: true });
	const tmp = join(tmpdir(), `git-fs-dl-${process.pid}-${Date.now()}`);
	mkdirSync(tmp, { recursive: true });
	const archive = join(tmp, info.artifact);
	const url = `https://github.com/${REPO}/releases/latest/download/${info.artifact}`;

	try {
		await downloadTo(url, archive);
		extract(archive, tmp, info.kind);
		// Release archives contain the bare binaries at the top level.
		const srcCli = join(tmp, `git-fs${info.ext}`);
		const srcMcp = join(tmp, `git-fs-mcp${info.ext}`);
		if (!existsSync(srcCli) || !existsSync(srcMcp)) {
			throw new Error(`extracted archive missing expected binaries (looked for git-fs${info.ext} and git-fs-mcp${info.ext} in ${tmp})`);
		}
		renameSync(srcCli, cli);
		renameSync(srcMcp, mcp);
		if (info.ext === '') {
			chmodSync(cli, 0o755);
			chmodSync(mcp, 0o755);
		}
		log(`installed binaries -> ${BIN_DIR}`);
	} finally {
		try { rmSync(tmp, { recursive: true, force: true }); } catch {}
	}
	return { cli, mcp };
}

function execBinary(bin, args, env) {
	const child = spawn(bin, args, {
		stdio: 'inherit',
		env: { ...process.env, ...env },
	});
	child.on('exit', (code, signal) => {
		if (signal) process.kill(process.pid, signal);
		else process.exit(code ?? 0);
	});
	child.on('error', (err) => {
		log(`failed to spawn ${bin}: ${err.message}`);
		process.exit(1);
	});
}

function isGitRepo(cwd) {
	const g = join(cwd, '.git');
	return existsSync(g);
}

function autoInitProject(cli, cwd) {
	if (!isGitRepo(cwd)) return;
	if (existsSync(join(cwd, '.git-fs'))) return;
	log(`auto-initializing git-fs in ${cwd}`);
	const r = spawnSync(cli, ['init-project'], { cwd, stdio: 'inherit' });
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

async function main() {
	const argv = process.argv.slice(2);
	const mode = argv[0];

	if (!mode) {
		log('usage: launcher.js <mcp|hook session-start|hook stop|hook read>');
		process.exit(2);
	}

	const { cli, mcp } = await ensureBinaries();

	if (mode === 'mcp') {
		execBinary(mcp, []);
		return;
	}

	if (mode === 'hook') {
		const sub = argv[1];
		if (!sub) { log('hook needs subcommand'); process.exit(2); }
		const cwd = process.cwd();
		if (sub === 'session-start') autoInitProject(cli, cwd);
		execBinary(cli, ['hook', sub]);
		return;
	}

	log(`unknown mode: ${mode}`);
	process.exit(2);
}

main().catch((err) => {
	log(`fatal: ${err.message}`);
	process.exit(1);
});
