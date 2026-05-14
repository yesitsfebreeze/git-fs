// Bundle CLI + MCP entries with esbuild. Cross-platform; works around
// PowerShell single-quote unfriendliness in inline npm scripts.

import { build } from "esbuild";

// Shim require() so bundled CJS deps (safe-buffer, sha.js inside
// isomorphic-git) keep working under ESM output.
const banner = {
  js:
    "#!/usr/bin/env node\n" +
    "import { createRequire as __cR } from 'module';\n" +
    "const require = __cR(import.meta.url);",
};

// Two builds:
//   1. local: git-fs/dist/{cli,mcp}.js — for `npm run dev` and bench
//   2. plugin: ../dist/{cli,mcp}.js — picked up by the Claude Code plugin
//      shipped from the repo root.
for (const outdir of ["dist", "../dist"]) {
  await build({
    entryPoints: ["src/cli.ts", "src/mcp.ts"],
    bundle: true,
    platform: "node",
    format: "esm",
    target: "node20",
    outdir,
    banner,
    logLevel: "info",
  });
}
