// src/mcp.mjs — hand-rolled stdio JSON-RPC MCP server (§10).
// Frame handling supports BOTH newline-delimited JSON (what Claude Code's stdio
// transport uses) and LSP-style Content-Length framing (spec §10). It detects
// per-buffer which one the peer speaks.

import * as store from "./store.mjs";

const SERVER_INFO = { name: "git-fs", version: "0.1.0" };

// Default branch comes from the session env; tools may override via args.branch.
function branchOf(args) {
  return (args && args.branch) || process.env.GIT_FS_BRANCH || store.currentBranch() || "main";
}

function textResult(text) {
  return { content: [{ type: "text", text: typeof text === "string" ? text : JSON.stringify(text, null, 2) }] };
}

// ── tool registry ───────────────────────────────────────────────────────────
const TOOLS = {
  git_fs_read: {
    description: "Read a file from the session branch; falls back to the working-tree bytes for untouched paths.",
    schema: { type: "object", properties: { branch: { type: "string" }, path: { type: "string" }, start: { type: "number" }, end: { type: "number" } }, required: ["path"] },
    run(a) {
      const br = branchOf(a);
      if (a.start != null || a.end != null) return textResult(store.readFileLines(br, a.path, a.start ?? 1, a.end));
      return textResult(store.readText(br, a.path));
    },
  },
  git_fs_write: {
    description: "Write (create or overwrite) a file on the session branch.",
    schema: { type: "object", properties: { branch: { type: "string" }, path: { type: "string" }, content: { type: "string" } }, required: ["path", "content"] },
    run(a) { const c = store.writeFile(branchOf(a), a.path, Buffer.from(a.content, "utf8"), a.message || `write ${a.path}`); return textResult(`wrote ${a.path} @ ${c}`); },
  },
  git_fs_replace: {
    description: "Replace exactly one occurrence of a string in a file on the session branch.",
    schema: { type: "object", properties: { branch: { type: "string" }, path: { type: "string" }, old: { type: "string" }, new: { type: "string" } }, required: ["path", "old", "new"] },
    run(a) { const c = store.replaceFile(branchOf(a), a.path, a.old, a.new, a.message); return textResult(`replaced in ${a.path} @ ${c}`); },
  },
  git_fs_patch: {
    description: "Splice lines [start,end] (1-based inclusive) of a file with new content.",
    schema: { type: "object", properties: { branch: { type: "string" }, path: { type: "string" }, start: { type: "number" }, end: { type: "number" }, content: { type: "string" } }, required: ["path", "start", "end", "content"] },
    run(a) { const c = store.patchFile(branchOf(a), a.path, a.start, a.end, a.content, a.message); return textResult(`patched ${a.path} @ ${c}`); },
  },
  git_fs_ls: {
    description: "List the touched files (deltas) on a branch.",
    schema: { type: "object", properties: { branch: { type: "string" }, prefix: { type: "string" }, recursive: { type: "boolean" } }, required: [] },
    run(a) { return textResult(store.listFiles(branchOf(a), a.prefix || "", a.recursive !== false)); },
  },
  git_fs_rm: {
    description: "Remove a file: from the branch tree if tracked, and tombstone it so Stop unlinks any disk copy.",
    schema: { type: "object", properties: { branch: { type: "string" }, path: { type: "string" } }, required: ["path"] },
    run(a) { const c = store.rm(branchOf(a), a.path, a.message); return textResult(`removed ${a.path} @ ${c}`); },
  },
  git_fs_log: {
    description: "Show the commit log of a branch.",
    schema: { type: "object", properties: { branch: { type: "string" } }, required: [] },
    run(a) { return textResult(store.log(branchOf(a))); },
  },
  git_fs_diff: {
    description: "Diff two refs (optionally restricted to a path).",
    schema: { type: "object", properties: { a: { type: "string" }, b: { type: "string" }, path: { type: "string" } }, required: ["a", "b"] },
    run(a) { return textResult(store.diff(a.a, a.b, a.path)); },
  },
  git_fs_branch_list: {
    description: "List all branches in the store.",
    schema: { type: "object", properties: {}, required: [] },
    run() { return textResult(store.branchList()); },
  },
  git_fs_merge: {
    description: "Merge two commits via git merge-tree; reports a clean commit or the conflicted paths.",
    schema: { type: "object", properties: { base: { type: "string" }, ours: { type: "string" }, theirs: { type: "string" } }, required: ["ours", "theirs"] },
    run(a) { return textResult(store.merge(a.base, a.ours, a.theirs)); },
  },
  git_fs_checkout: {
    description: "Materialize the session branch's touched files to disk (scoped to its seed). Refuses to overwrite on-disk content newer than git-fs unless force:true.",
    schema: { type: "object", properties: { branch: { type: "string" }, force: { type: "boolean" } }, required: [] },
    run(a) {
      const br = branchOf(a);
      const { conflicts } = store.checkout(br, { force: a.force === true });
      if (conflicts && conflicts.length) {
        return { content: [{ type: "text", text: `git-fs: REFUSED to overwrite ${conflicts.length} file(s) whose on-disk content is newer than the branch blob (pass force:true to override):\n` + conflicts.map((p) => `  ${p}`).join("\n") }], isError: true };
      }
      return textResult(`checked out ${br}`);
    },
  },
  // ── extras (handy; not in the §10 minimum) ──
  git_fs_init: {
    description: "Initialize the bare store and create the session branch (empty seed).",
    schema: { type: "object", properties: { branch: { type: "string" } }, required: [] },
    run(a) { store.ensureStore(); const br = branchOf(a); const seed = store.resolveTip(br) || store.branchCreate(br, null); return textResult(`store ready; ${br} @ ${seed}`); },
  },
  git_fs_branch_create: {
    description: "Create a branch, optionally from a commit (else an empty seed).",
    schema: { type: "object", properties: { name: { type: "string" }, from: { type: "string" } }, required: ["name"] },
    run(a) { return textResult(`created ${a.name} @ ${store.branchCreate(a.name, a.from || null)}`); },
  },
  git_fs_branch_delete: {
    description: "Delete a branch ref.",
    schema: { type: "object", properties: { name: { type: "string" } }, required: ["name"] },
    run(a) { return textResult(store.branchDelete(a.name) ? `deleted ${a.name}` : `no such branch ${a.name}`); },
  },
  git_fs_debug: {
    description: "Report store state for debugging.",
    schema: { type: "object", properties: { branch: { type: "string" } }, required: [] },
    run(a) { const br = branchOf(a); return textResult({ repo: process.env.GIT_FS_REPO || ".git-fs", disk: process.env.GIT_FS_DISK || process.cwd(), branch: br, tip: store.resolveTip(br), branches: store.branchList() }); },
  },
};

// ── JSON-RPC dispatch ───────────────────────────────────────────────────────
function dispatch(msg) {
  const { id, method, params } = msg;
  const reply = (result) => ({ jsonrpc: "2.0", id, result });
  const fail = (code, message) => ({ jsonrpc: "2.0", id, error: { code, message } });

  try {
    switch (method) {
      case "initialize":
        return reply({
          protocolVersion: (params && params.protocolVersion) || "2024-11-05",
          capabilities: { tools: {} },
          serverInfo: SERVER_INFO,
        });
      case "tools/list":
        return reply({
          tools: Object.entries(TOOLS).map(([name, t]) => ({ name, description: t.description, inputSchema: t.schema })),
        });
      case "tools/call": {
        const name = params && params.name;
        const t = TOOLS[name];
        if (!t) return fail(-32601, `Unknown tool: ${name}`);
        try {
          return reply(t.run((params && params.arguments) || {}));
        } catch (e) {
          // tool errors surface as a tool result with isError, per MCP convention
          return reply({ content: [{ type: "text", text: String(e && e.message || e) }], isError: true });
        }
      }
      case "ping":
        return reply({});
      default:
        if (id === undefined) return null; // notification — no response
        return fail(-32601, `Method not found: ${method}`);
    }
  } catch (e) {
    return fail(-32603, String(e && e.message || e));
  }
}

// ── framing ─────────────────────────────────────────────────────────────────
export function run() {
  let buf = Buffer.alloc(0);
  const send = (obj) => {
    if (!obj) return;
    const json = JSON.stringify(obj);
    // Claude Code stdio expects newline-delimited JSON.
    process.stdout.write(json + "\n");
  };

  process.stdin.on("data", (chunk) => {
    buf = Buffer.concat([buf, chunk]);
    // Content-Length framed?
    for (;;) {
      const s = buf.toString("utf8");
      if (s.startsWith("Content-Length:")) {
        const headerEnd = s.indexOf("\r\n\r\n");
        if (headerEnd === -1) return;
        const m = s.slice(0, headerEnd).match(/Content-Length:\s*(\d+)/i);
        if (!m) { buf = Buffer.alloc(0); return; }
        const len = parseInt(m[1], 10);
        const bodyStart = headerEnd + 4;
        if (buf.length < bodyStart + len) return;
        const body = buf.slice(bodyStart, bodyStart + len).toString("utf8");
        buf = buf.slice(bodyStart + len);
        try { send(dispatch(JSON.parse(body))); } catch {}
        continue;
      }
      // newline-delimited
      const nl = buf.indexOf(0x0a);
      if (nl === -1) return;
      const line = buf.slice(0, nl).toString("utf8").trim();
      buf = buf.slice(nl + 1);
      if (!line) continue;
      try { send(dispatch(JSON.parse(line))); } catch {}
    }
  });

  process.stdin.on("end", () => process.exit(0));
}
