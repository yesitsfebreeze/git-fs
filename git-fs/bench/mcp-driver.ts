/**
 * MCP-driven benchmark.
 *
 * Spawns dist/mcp.js (or a custom command), sends the 500/500/200/1/1
 * JSON-RPC workload over stdio, prints per-phase wall-clock.
 *
 * Usage:
 *   tsx bench/mcp-driver.ts                # default: node dist/mcp.js
 *   tsx bench/mcp-driver.ts --cmd "<exe>"  # custom executable
 */

import { spawn, ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";
import { performance } from "node:perf_hooks";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const N_FILES = 500;
const N_READS = 200;

interface Args {
  cmd: string;
  args: string[];
  label: string;
  repo: string;
}

function parseArgs(): Args {
  const argv = process.argv.slice(2);
  let custom: string | null = null;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--cmd") custom = argv[++i] ?? null;
  }
  const repoBase = fs.mkdtempSync(path.join(os.tmpdir(), "gitfs-mcp-bench-"));
  const repo = path.join(repoBase, "repo");
  process.env["GIT_FS_REPO"] = repo;

  if (custom) return { cmd: custom, args: [], label: custom, repo };
  return {
    cmd: process.execPath,
    args: [path.resolve("dist", "mcp.js")],
    label: "node dist/mcp.js",
    repo,
  };
}

class Rpc {
  private nextId = 1;
  private pending = new Map<number, (v: unknown, err: unknown) => void>();
  private proc: ChildProcessWithoutNullStreams;

  constructor(proc: ChildProcessWithoutNullStreams) {
    this.proc = proc;
    const rl = createInterface({ input: proc.stdout });
    rl.on("line", (line) => {
      const t = line.trim();
      if (!t) return;
      let msg: { id?: number; result?: unknown; error?: { message: string } };
      try { msg = JSON.parse(t); } catch { return; }
      if (msg.id === undefined) return;
      const pend = this.pending.get(msg.id);
      if (!pend) return;
      this.pending.delete(msg.id);
      if (msg.error) pend(null, new Error(msg.error.message));
      else pend(msg.result, null);
    });
    proc.stderr.on("data", () => { /* swallow */ });
  }

  call(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolveP, rejectP) => {
      this.pending.set(id, (v, e) => (e ? rejectP(e) : resolveP(v)));
      this.proc.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
    });
  }

  async tool(name: string, args: Record<string, unknown>): Promise<string> {
    const result = (await this.call("tools/call", { name, arguments: args })) as {
      content: Array<{ type: string; text: string }>;
      isError: boolean;
    };
    if (result.isError) throw new Error(result.content[0]?.text ?? "tool error");
    return result.content[0]?.text ?? "";
  }

  shutdown() {
    this.proc.stdin.end();
  }
}

async function main() {
  const { cmd, args, label, repo } = parseArgs();
  console.log(`target: ${label}`);
  console.log(`repo:   ${repo}`);

  const proc = spawn(cmd, args, { env: process.env, stdio: ["pipe", "pipe", "pipe"] });
  const rpc = new Rpc(proc);
  await rpc.call("initialize", {});
  await rpc.tool("git_fs_init", {});
  await rpc.tool("git_fs_branch_create", { name: "main" });
  await rpc.tool("git_fs_branch_create", { name: "agent/bench", from: "main" });

  const t0 = performance.now();

  const t1 = performance.now();
  for (let i = 0; i < N_FILES; i++) {
    await rpc.tool("git_fs_write", {
      branch: "agent/bench",
      path: `src/mod_${i}.ts`,
      content: `// file ${i}\nexport const x_${i} = ${i};\n`,
      message: `add mod_${i}`,
    });
  }
  const t2 = performance.now();

  for (let i = 0; i < N_FILES; i++) {
    await rpc.tool("git_fs_replace", {
      branch: "agent/bench",
      path: `src/mod_${i}.ts`,
      old_str: `export const x_${i} = ${i};`,
      new_str: `export const x_${i} = ${i * 2};`,
      message: `bump mod_${i}`,
    });
  }
  const t3 = performance.now();

  for (let i = 0; i < N_READS; i++) {
    if (i % 2 === 0) {
      await rpc.tool("git_fs_read", { ref: "agent/bench", path: `src/mod_${i}.ts` });
    } else {
      await rpc.tool("git_fs_read", {
        ref: "agent/bench",
        path: `src/mod_${i}.ts`,
        start_line: 1,
        end_line: 2,
      });
    }
  }
  const t4 = performance.now();

  await rpc.tool("git_fs_merge", {
    ours: "agent/bench",
    theirs: "main",
    base: "main",
    into: "main",
    message: "merge bench",
  });
  const t5 = performance.now();

  const dest = path.join(path.dirname(repo), "out");
  await rpc.tool("git_fs_checkout", { ref: "main", dest });
  const t6 = performance.now();

  rpc.shutdown();

  const fmt = (ms: number) => `${ms.toFixed(1).padStart(8)} ms`;
  console.log(`\nphase                  total       per-op`);
  console.log(`writes  (${N_FILES})        ${fmt(t2 - t1)}   ${fmt((t2 - t1) / N_FILES)}`);
  console.log(`replaces(${N_FILES})        ${fmt(t3 - t2)}   ${fmt((t3 - t2) / N_FILES)}`);
  console.log(`reads   (${N_READS})        ${fmt(t4 - t3)}   ${fmt((t4 - t3) / N_READS)}`);
  console.log(`merge   (1)          ${fmt(t5 - t4)}`);
  console.log(`checkout(1)          ${fmt(t6 - t5)}`);
  console.log(`TOTAL                ${fmt(t6 - t0)}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
