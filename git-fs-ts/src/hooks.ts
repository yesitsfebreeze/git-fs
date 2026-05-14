/**
 * Claude Code hook handlers — port of git-fs/src/main.rs::run_hook.
 *
 * Reads a JSON payload from stdin (Claude Code hook protocol), looks up the
 * session id, and dispatches the requested hook.
 */

import { Store } from "./store.js";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import picomatch from "picomatch";
import lockfile from "proper-lockfile";

const HARD_MERGEIGNORE = [".agent", "CONFLICTS.md"];

type HookAction = "session-start" | "post-write" | "post-edit" | "read" | "stop";

interface HookPayload {
  session_id?: string;
  model?: string;
  tool_input?: { file_path?: string };
}

function readStdinSync(): string {
  try {
    return fs.readFileSync(0, "utf-8");
  } catch {
    return "";
  }
}

function relPath(abs: string, cwd: string): string | null {
  const a = path.resolve(abs);
  const c = path.resolve(cwd);
  const rel = path.relative(c, a);
  if (rel.startsWith("..") || path.isAbsolute(rel)) return null;
  return rel.split(path.sep).join("/");
}

function makeIgnoreMatcher(patterns: string[]): (p: string) => boolean {
  if (patterns.length === 0) return () => false;
  const expanded = patterns.map((raw) => {
    const trimmed = raw.trim();
    if (trimmed.endsWith("/")) return `${trimmed.slice(0, -1)}/**`;
    return trimmed;
  });
  const matchers = expanded.map((p) =>
    picomatch(p, { dot: true, nocase: process.platform === "win32" }),
  );
  return (target: string) => matchers.some((m) => m(target));
}

async function loadMergeignore(store: Store, ref: string): Promise<(p: string) => boolean> {
  const patterns: string[] = [...HARD_MERGEIGNORE];
  try {
    const bytes = await store.readFile(ref, ".git-fs/mergeignore");
    const text = new TextDecoder("utf-8").decode(bytes);
    for (const raw of text.split(/\r?\n/)) {
      const line = raw.trim();
      if (line.length === 0 || line.startsWith("#")) continue;
      patterns.push(line);
    }
  } catch {
    // file absent — only hard defaults
  }
  return makeIgnoreMatcher(patterns);
}

async function stripMergeignored(
  store: Store,
  branch: string,
  match: (p: string) => boolean,
): Promise<number> {
  const entries = await store.listFiles(branch, "", true);
  const paths = entries.filter((e) => e.kind === "blob" && match(e.path)).map((e) => e.path);
  if (paths.length === 0) return 0;
  await store.removePaths(branch, paths, `strip mergeignored (${paths.length} path(s))`);
  return paths.length;
}

async function reportSiblingOverlap(
  store: Store,
  ours: string,
  match: (p: string) => boolean,
): Promise<void> {
  let ourPaths: Set<string>;
  try {
    ourPaths = new Set((await store.changedPaths("main", ours)).filter((p) => !match(p)));
  } catch {
    return;
  }
  if (ourPaths.size === 0) return;

  const branches = await store.branchList();
  for (const sib of branches) {
    if (sib === ours || !sib.startsWith("agent/")) continue;
    let sibPaths: string[];
    try {
      sibPaths = await store.changedPaths("main", sib);
    } catch {
      continue;
    }
    const overlap = sibPaths.filter((p) => !match(p) && ourPaths.has(p));
    if (overlap.length === 0) continue;
    process.stderr.write(`git-fs: sibling overlap with ${sib} on ${overlap.length} path(s):\n`);
    for (const p of overlap) process.stderr.write(`  ${p}\n`);
  }
}

async function withMergeLock<T>(repo: string, fn: () => Promise<T>): Promise<T> {
  const lockPath = path.join(repo, "merge.lock");
  if (!fs.existsSync(lockPath)) fs.writeFileSync(lockPath, "");
  const release = await lockfile.lock(lockPath, { retries: { retries: 50, minTimeout: 50, maxTimeout: 200 } });
  try {
    return await fn();
  } finally {
    await release();
  }
}

export async function runHook(action: HookAction): Promise<void> {
  const repo = process.env["GIT_FS_REPO"] ?? ".git-fs";
  if (!fs.existsSync(repo)) return; // hook is a no-op when git-fs missing

  const cwd = process.cwd();
  const raw = readStdinSync();
  let payload: HookPayload = {};
  try {
    payload = JSON.parse(raw) as HookPayload;
  } catch {
    return;
  }
  const sessionId = payload.session_id;
  if (typeof sessionId !== "string") return;
  const branch = `agent/${sessionId}`;
  const store = await Store.open(repo);

  switch (action) {
    case "session-start": {
      const model = payload.model ?? "unknown";
      let baseOid = "";
      let from: string | undefined;
      try {
        baseOid = await store.resolveCommit("main");
        from = "main";
      } catch {
        from = undefined;
      }
      try {
        await store.branchCreate(branch, from);
      } catch {
        // already exists — fine
      }
      await store.writeFile(
        branch,
        ".agent",
        Buffer.from(`model: ${model}\nsession: ${sessionId}\nbase: ${baseOid}\n`, "utf-8"),
        `session start\n\nmodel: ${model}`,
      );
      const intent =
        `# Session intent\n\n` +
        `session: ${sessionId}\n` +
        `model: ${model}\n\n` +
        `## What this session is doing\n\n\n` +
        `## Why\n\n\n` +
        `## Paths touched\n\n(see .git-fs/session/paths.md)\n`;
      try {
        await store.writeFile(branch, ".git-fs/session/intent.md", Buffer.from(intent, "utf-8"), "seed session intent");
      } catch {
        // intent seed is best-effort
      }
      process.stderr.write(`git-fs: branch '${branch}' ready (model: ${model})\n`);
      process.stdout.write(
        `git-fs session active.\n` +
          `Branch: ${branch}\n` +
          `All file changes are tracked automatically via hooks.\n` +
          `Prefer git_fs_replace for edits (text-match, immune to line-number drift).\n` +
          `Use git_fs_write for new files, git_fs_read to read, git_fs_patch for line-range edits.\n` +
          `Use git_fs_log branch:${branch} to see your change history.\n` +
          `Use git_fs_branch_list to see all agent sessions.\n` +
          `Load schemas upfront: ToolSearch select:git_fs_write,git_fs_read,git_fs_replace,git_fs_patch,git_fs_ls,git_fs_rm,git_fs_merge,git_fs_diff,git_fs_log,git_fs_branch_create,git_fs_branch_list,git_fs_branch_delete,git_fs_checkout\n`,
      );
      return;
    }

    case "post-write":
    case "post-edit": {
      const filePath = payload.tool_input?.file_path;
      if (!filePath) return;
      const rel = relPath(filePath, cwd);
      if (!rel) return;
      if (!fs.existsSync(filePath)) return;
      const content = fs.readFileSync(filePath);
      const op = action === "post-write" ? "write" : "edit";
      try {
        await store.writeFile(branch, rel, content, `${op} ${rel}`);
        process.stderr.write(`git-fs: ${op} ${branch}:${rel}\n`);
      } catch (e) {
        process.stderr.write(`git-fs: ${op} ${rel} failed: ${e instanceof Error ? e.message : String(e)}\n`);
      }
      return;
    }

    case "read": {
      const filePath = payload.tool_input?.file_path;
      if (!filePath) return;
      const rel = relPath(filePath, cwd);
      if (!rel) return;
      try {
        const bytes = await store.readFile(branch, rel);
        const parent = path.dirname(filePath);
        fs.mkdirSync(parent, { recursive: true });
        fs.writeFileSync(filePath, bytes);
      } catch {
        // ref/path missing — let native Read fall through
      }
      return;
    }

    case "stop": {
      const branches = await store.branchList();
      if (!branches.includes("main")) {
        // No main yet — bootstrap from agent
        try {
          const agentOid = await store.resolveCommit(branch);
          await store.writeBranchRef("main", agentOid);
          try {
            await store.checkout("main", cwd);
            process.stderr.write(`git-fs: bootstrapped main from ${branch}; materialized → ${cwd}\n`);
          } catch (e) {
            process.stderr.write(`git-fs: checkout failed: ${e instanceof Error ? e.message : String(e)}\n`);
          }
        } catch (e) {
          process.stderr.write(`git-fs: bootstrap failed: ${e instanceof Error ? e.message : String(e)}\n`);
        }
        return;
      }

      let base = branch;
      try {
        const bytes = await store.readFile(branch, ".agent");
        const text = new TextDecoder("utf-8").decode(bytes);
        const m = text.split(/\r?\n/).find((l) => l.startsWith("base: "));
        if (m) {
          const v = m.slice("base: ".length).trim();
          if (v.length > 0) base = v;
        }
      } catch {
        // missing .agent — fall back to branch (no base ancestor known)
      }

      const ignore = await loadMergeignore(store, "main");
      try {
        const n = await stripMergeignored(store, branch, ignore);
        if (n > 0) process.stderr.write(`git-fs: stripped ${n} mergeignored path(s) from ${branch}\n`);
      } catch (e) {
        process.stderr.write(`git-fs: strip mergeignored failed: ${e instanceof Error ? e.message : String(e)}\n`);
      }

      try {
        await reportSiblingOverlap(store, branch, ignore);
      } catch (e) {
        process.stderr.write(`git-fs: sibling reconcile failed: ${e instanceof Error ? e.message : String(e)}\n`);
      }

      try {
        await withMergeLock(repo, async () => {
          const result = await store.merge(base, branch, "main", "main", `merge from ${branch}`);
          if (result.kind === "clean") {
            if (result.commitOid) process.stderr.write(`git-fs: merged ${branch} → main\n`);
            try {
              await store.checkout("main", cwd);
              process.stderr.write(`git-fs: materialized main → ${cwd}\n`);
            } catch (e) {
              process.stderr.write(`git-fs: checkout failed: ${e instanceof Error ? e.message : String(e)}\n`);
            }
          } else {
            let report = `# Conflicts: ${branch} → main\n\n`;
            for (const c of result.conflicts) {
              report += `## ${c.path}\n\nours:\n\`\`\`\n${c.ours ?? "(deleted)"}\n\`\`\`\n\ntheirs:\n\`\`\`\n${c.theirs ?? "(deleted)"}\n\`\`\`\n\n`;
            }
            try {
              await store.writeFile(branch, "CONFLICTS.md", Buffer.from(report, "utf-8"), "conflict report");
            } catch {
              // best-effort
            }
            process.stderr.write(`git-fs: conflicts on merge ${branch} → main; see CONFLICTS.md\n`);
          }
        });
      } catch (e) {
        process.stderr.write(`git-fs: merge lock failed: ${e instanceof Error ? e.message : String(e)}\n`);
      }
      return;
    }
  }
}

// ── duration parsing (for prune) ─────────────────────────────────────────────

export function parseDurationSecs(input: string): number {
  const s = input.trim();
  if (s.length === 0) throw new Error("empty duration");
  const last = s.charAt(s.length - 1);
  if (/[0-9]/.test(last)) {
    const n = Number(s);
    if (!Number.isFinite(n)) throw new Error(`bad duration '${s}'`);
    return n;
  }
  const num = Number(s.slice(0, -1));
  if (!Number.isFinite(num)) throw new Error(`bad duration '${s}'`);
  switch (last) {
    case "s": return num;
    case "m": return num * 60;
    case "h": return num * 3600;
    case "d": return num * 86_400;
    case "w": return num * 86_400 * 7;
    default: throw new Error(`unknown duration unit '${last}' in '${s}'`);
  }
}

// ── init-project ─────────────────────────────────────────────────────────────

export async function initProject(repoPath: string, mcpConfigPath: string): Promise<void> {
  if (fs.existsSync(repoPath)) {
    try { await Store.open(repoPath); }
    catch { await Store.init(repoPath); }
  } else {
    await Store.init(repoPath);
  }
  const repoAbs = path.resolve(repoPath);
  let cfg: Record<string, unknown> = {};
  if (fs.existsSync(mcpConfigPath)) {
    try {
      cfg = JSON.parse(fs.readFileSync(mcpConfigPath, "utf-8")) as Record<string, unknown>;
    } catch {
      cfg = {};
    }
  }
  const servers = (cfg["mcpServers"] as Record<string, unknown> | undefined) ?? {};
  servers["git-fs"] = {
    command: "git-fs-mcp",
    env: { GIT_FS_REPO: repoAbs.split(path.sep).join("/") },
  };
  cfg["mcpServers"] = servers;
  fs.writeFileSync(mcpConfigPath, JSON.stringify(cfg, null, 2) + "\n");
  process.stdout.write(`Repo:     ${repoAbs}\n`);
  process.stdout.write(`Config:   ${mcpConfigPath}\n\n`);
  process.stdout.write(`Restart Claude Code to load the MCP server.\n`);
  process.stdout.write(`Tools: git_fs_write, git_fs_read, git_fs_ls, git_fs_merge, git_fs_diff, git_fs_log ...\n`);
  // silence unused
  void os;
}

// ── prune ────────────────────────────────────────────────────────────────────

export interface PruneOptions {
  merged: boolean;
  olderThan: string | null;
  into: string;
  prefix: string;
  apply: boolean;
  json: boolean;
}

export async function runPrune(repoPath: string, opts: PruneOptions): Promise<void> {
  const store = await Store.open(repoPath);
  const cutoff = opts.olderThan ? parseDurationSecs(opts.olderThan) : null;
  const now = Math.floor(Date.now() / 1000);

  const branches = await store.branchList();
  type Row = { branch: string; ageSecs: number; merged: boolean };
  const report: Row[] = [];

  for (const b of branches) {
    if (!b.startsWith(opts.prefix) || b === opts.into) continue;
    let age: number;
    try { age = now - (await store.tipTime(b)); }
    catch { continue; }
    if (cutoff !== null && age < cutoff) continue;
    let merged = false;
    try { merged = await store.isMergedInto(b, opts.into); } catch { merged = false; }
    if (opts.merged && !merged) continue;
    report.push({ branch: b, ageSecs: age, merged });
  }

  const deleted: string[] = [];
  if (opts.apply) {
    for (const row of report) {
      try { await store.branchDelete(row.branch); deleted.push(row.branch); }
      catch (e) { process.stderr.write(`prune: failed to delete ${row.branch}: ${e instanceof Error ? e.message : String(e)}\n`); }
    }
  }

  if (opts.json) {
    process.stdout.write(JSON.stringify({
      applied: opts.apply,
      candidates: report.map((r) => ({
        branch: r.branch,
        age_secs: r.ageSecs,
        merged: r.merged,
        deleted: opts.apply && deleted.includes(r.branch),
      })),
    }) + "\n");
    return;
  }
  if (report.length === 0) {
    process.stdout.write("nothing to prune\n");
    return;
  }
  const verb = opts.apply ? "deleted" : "would delete";
  process.stdout.write(`${verb} ${report.length} branch(es):\n`);
  for (const r of report) {
    const tag = r.merged ? "merged" : "UNMERGED";
    process.stdout.write(`  ${r.branch}  age=${r.ageSecs}s  ${tag}\n`);
  }
  if (!opts.apply) process.stdout.write("\n(dry run — pass --apply to delete)\n");
}
