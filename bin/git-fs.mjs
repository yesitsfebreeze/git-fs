#!/usr/bin/env node
// bin/git-fs.mjs — single entry. Dispatches on argv[2]:
//   mcp           → stdio JSON-RPC MCP server
//   hook <action> → lifecycle hook handler (reads JSON payload on stdin)
//   <subcommand>  → tiny CLI over the store (read/ls/log/write/materialize)

const [, , sub, ...rest] = process.argv;

if (sub === "mcp") {
  const { run } = await import(new URL("../src/mcp.mjs", import.meta.url));
  run();
} else if (sub === "hook") {
  const { runHook } = await import(new URL("../src/hooks.mjs", import.meta.url));
  runHook(rest[0]);
} else {
  const store = await import(new URL("../src/store.mjs", import.meta.url));
  const branch = process.env.GIT_FS_BRANCH || "main";
  const out = (v) => process.stdout.write((typeof v === "string" ? v : JSON.stringify(v, null, 2)) + "\n");
  try {
    switch (sub) {
      case "read": out(store.readText(branch, rest[0])); break;
      case "ls": out(store.listFiles(branch, rest[0] || "", true).join("\n")); break;
      case "log": out(store.log(branch)); break;
      case "branches": out(store.branchList().join("\n")); break;
      case "write": {
        const fs = await import("node:fs");
        const bytes = rest[1] ? fs.readFileSync(rest[1]) : fs.readFileSync(0);
        out(store.writeFile(branch, rest[0], bytes, `write ${rest[0]}`));
        break;
      }
      case "materialize":
      case "checkout": store.ensureStore(); store.checkout(branch); out(`materialized ${branch}`); break;
      case "init": store.ensureStore(); out(store.resolveTip(branch) || store.branchCreate(branch, null)); break;
      case undefined:
      case "help":
        out("usage: git-fs <mcp | hook <action> | read|ls|log|branches|write|materialize|init> [args]\n  env: GIT_FS_REPO (store, default .git-fs), GIT_FS_DISK (worktree), GIT_FS_BRANCH (default main)");
        break;
      default:
        process.stderr.write(`git-fs: unknown subcommand '${sub}'\n`); process.exit(2);
    }
  } catch (e) {
    process.stderr.write(`git-fs: ${String(e && e.message || e)}\n`); process.exit(1);
  }
}
