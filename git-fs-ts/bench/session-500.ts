/**
 * 500-commit synthetic session benchmark.
 *
 * Simulates a typical agent session:
 *   - 500 writes (new files)
 *   - 500 replaces (text edits)
 *   - 200 reads (full + slice)
 *   - 1 final merge agent → main
 *   - 1 checkout main → dest
 *
 * Outputs total wall-clock time and per-op average. Run separately against
 * the Rust git-fs binary to derive the ratio. The 2× / 5× decision gate
 * lives in git-fs-ts/README.md.
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

  // 500 writes
  const t1 = performance.now();
  for (let i = 0; i < N_FILES; i++) {
    const filepath = `src/mod_${i}.ts`;
    const content = `// file ${i}\nexport const x_${i} = ${i};\n`;
    await store.writeFile("agent/bench", filepath, Buffer.from(content, "utf-8"), `add mod_${i}`);
  }
  const t2 = performance.now();

  // 500 replaces
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

  // 200 reads (mix of full and slice)
  for (let i = 0; i < N_READS; i++) {
    const filepath = `src/mod_${i}.ts`;
    if (i % 2 === 0) {
      await store.readFile("agent/bench", filepath);
    } else {
      await store.readFileLines("agent/bench", filepath, 1, 2);
    }
  }
  const t4 = performance.now();

  // 1 merge agent → main
  const mergeResult = await store.merge("main", "agent/bench", "main", "main", "merge bench");
  if (mergeResult.kind !== "clean") throw new Error("unexpected conflicts");
  const t5 = performance.now();

  // 1 checkout
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

  // Equivalent Rust invocation written to a sibling .sh for manual comparison.
  const cmpScript = `#!/usr/bin/env bash
# Manual comparison runner. Run after \`cargo build --release -p git-fs\`.
set -euo pipefail
TMP=\$(mktemp -d)
REPO="\$TMP/repo"
target/release/git-fs --repo "\$REPO" init
target/release/git-fs --repo "\$REPO" branch create main
target/release/git-fs --repo "\$REPO" branch create agent/bench --from main

time (
  for i in \$(seq 0 ${N_FILES - 1}); do
    printf '// file %s\\nexport const x_%s = %s;\\n' "\$i" "\$i" "\$i" \\
      | target/release/git-fs --repo "\$REPO" write agent/bench "src/mod_\${i}.ts" --message "add mod_\${i}" >/dev/null
  done
)
echo "(replaces/reads/merge/checkout omitted from script; instrument with hyperfine if needed)"
rm -rf "\$TMP"
`;
  const scriptPath = path.join(tmp, "rust-equivalent.sh");
  fs.writeFileSync(scriptPath, cmpScript, { mode: 0o755 });
  console.log(`\nRust comparator scaffold written to: ${scriptPath}`);
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
