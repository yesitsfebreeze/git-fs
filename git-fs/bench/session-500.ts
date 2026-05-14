/**
 * 500-commit synthetic session benchmark (in-process).
 *
 * Drives the Store directly (no MCP IPC). Use bench/mcp-driver.ts to
 * benchmark the end-to-end stdio path.
 *
 * Workload:
 *   - 500 writes (new files)
 *   - 500 replaces (text edits)
 *   - 200 reads (full + slice)
 *   - 1 merge agent → main
 *   - 1 checkout main → dest
 */

import { Store } from "../src/store.js";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";

const N_FILES = 500;
const N_READS = 200;

async function run() {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "gitfs-bench-"));
  const repo = path.join(tmp, "repo");
  console.log(`repo: ${repo}`);
  const store = await Store.init(repo);
  await store.branchCreate("main");
  await store.branchCreate("agent/bench", "main");

  const t0 = performance.now();

  const t1 = performance.now();
  for (let i = 0; i < N_FILES; i++) {
    const filepath = `src/mod_${i}.ts`;
    const content = `// file ${i}\nexport const x_${i} = ${i};\n`;
    await store.writeFile("agent/bench", filepath, Buffer.from(content, "utf-8"), `add mod_${i}`);
  }
  const t2 = performance.now();

  for (let i = 0; i < N_FILES; i++) {
    const filepath = `src/mod_${i}.ts`;
    await store.replaceFile(
      "agent/bench",
      filepath,
      `export const x_${i} = ${i};`,
      `export const x_${i} = ${i * 2};`,
      `bump mod_${i}`,
    );
  }
  const t3 = performance.now();

  for (let i = 0; i < N_READS; i++) {
    const filepath = `src/mod_${i}.ts`;
    if (i % 2 === 0) {
      await store.readFile("agent/bench", filepath);
    } else {
      await store.readFileLines("agent/bench", filepath, 1, 2);
    }
  }
  const t4 = performance.now();

  const mergeResult = await store.merge("main", "agent/bench", "main", "main", "merge bench");
  if (mergeResult.kind !== "clean") throw new Error("unexpected conflicts");
  const t5 = performance.now();

  const dest = path.join(tmp, "out");
  await store.checkout("main", dest);
  const t6 = performance.now();

  const fmt = (ms: number) => `${ms.toFixed(1).padStart(8)} ms`;
  console.log(`\nphase                  total       per-op`);
  console.log(`writes  (${N_FILES})        ${fmt(t2 - t1)}   ${fmt((t2 - t1) / N_FILES)}`);
  console.log(`replaces(${N_FILES})        ${fmt(t3 - t2)}   ${fmt((t3 - t2) / N_FILES)}`);
  console.log(`reads   (${N_READS})        ${fmt(t4 - t3)}   ${fmt((t4 - t3) / N_READS)}`);
  console.log(`merge   (1)          ${fmt(t5 - t4)}`);
  console.log(`checkout(1)          ${fmt(t6 - t5)}`);
  console.log(`TOTAL                ${fmt(t6 - t0)}`);
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
