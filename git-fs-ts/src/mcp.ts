/**
 * git-fs-mcp — stdio JSON-RPC MCP server.
 *
 * Mirrors git-fs/src/bin/mcp.rs tool surface 1:1. Hand-rolled JSON-RPC
 * loop (no MCP SDK dependency) for parity with the Rust impl which also
 * writes raw frames to stdout.
 */

import { Store, MergeResult } from "./store.js";
import { Buffer } from "node:buffer";
import { createInterface } from "node:readline";

const PROTOCOL_VERSION = "2024-11-05";
const SERVER_NAME = "git-fs-mcp";
const SERVER_VERSION = "2.0.0-spike.0";

const REPO = () => process.env["GIT_FS_REPO"] ?? ".git-fs";

interface JsonRpcMessage {
  jsonrpc: "2.0";
  id?: string | number | null;
  method?: string;
  params?: unknown;
}

function err(id: unknown, code: number, message: string) {
  return JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } });
}

function ok(id: unknown, result: unknown) {
  return JSON.stringify({ jsonrpc: "2.0", id, result });
}

async function dispatch(method: string, params: Record<string, unknown>): Promise<unknown> {
  switch (method) {
    case "initialize":
      return {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: { name: SERVER_NAME, version: SERVER_VERSION },
      };
    case "ping":
      return {};
    case "tools/list":
      return { tools: toolSchemas() };
    case "tools/call": {
      const name = String(params["name"] ?? "");
      const args = (params["arguments"] ?? {}) as Record<string, unknown>;
      try {
        const text = await callTool(name, args);
        return { content: [{ type: "text", text }], isError: false };
      } catch (e) {
        return {
          content: [{ type: "text", text: e instanceof Error ? e.message : String(e) }],
          isError: true,
        };
      }
    }
    default:
      throw new Error(`unknown method: ${method}`);
  }
}

function commitJson(oid: string, branch: string, filepath: string): string {
  return JSON.stringify({ commit: oid, branch, path: filepath });
}

async function callTool(name: string, a: Record<string, unknown>): Promise<string> {
  const repo = REPO();
  if (name === "git_fs_init") {
    await Store.init(repo);
    return `Initialized repo: ${repo}`;
  }
  const store = await Store.open(repo);
  const s = (k: string) => {
    const v = a[k];
    if (typeof v !== "string") throw new Error(`${k} required`);
    return v;
  };
  const optS = (k: string) => (typeof a[k] === "string" ? (a[k] as string) : undefined);
  const optN = (k: string) => (typeof a[k] === "number" ? (a[k] as number) : undefined);
  const optB = (k: string) => (typeof a[k] === "boolean" ? (a[k] as boolean) : undefined);

  switch (name) {
    case "git_fs_branch_create":
      await store.branchCreate(s("name"), optS("from"));
      return `Created branch: ${s("name")}`;

    case "git_fs_branch_list":
      return (await store.branchList()).join("\n");

    case "git_fs_branch_delete":
      await store.branchDelete(s("name"));
      return `Deleted branch: ${s("name")}`;

    case "git_fs_write": {
      const oid = await store.writeFile(
        s("branch"),
        s("path"),
        Buffer.from(s("content"), "utf-8"),
        optS("message") ?? "write",
      );
      return commitJson(oid, s("branch"), s("path"));
    }

    case "git_fs_read": {
      const start = optN("start_line");
      const end = optN("end_line");
      if (start !== undefined || end !== undefined) {
        return store.readFileLines(s("ref"), s("path"), start, end);
      }
      const bytes = await store.readFile(s("ref"), s("path"));
      return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    }

    case "git_fs_patch": {
      const oid = await store.patchFile(
        s("branch"),
        s("path"),
        Number(a["start_line"]),
        Number(a["end_line"]),
        s("content"),
        optS("message") ?? "patch",
      );
      return commitJson(oid, s("branch"), s("path"));
    }

    case "git_fs_replace": {
      const oid = await store.replaceFile(
        s("branch"),
        s("path"),
        s("old_str"),
        s("new_str"),
        optS("message") ?? "replace",
      );
      return commitJson(oid, s("branch"), s("path"));
    }

    case "git_fs_rm": {
      const branch = s("branch");
      const filepath = s("path");
      const msg = optS("message") || `rm ${filepath}`;
      const oid = await store.removeFile(branch, filepath, msg);
      return JSON.stringify({ commit: oid });
    }

    case "git_fs_ls": {
      const entries = await store.listFiles(s("ref"), optS("path") ?? "", optB("recursive") ?? false);
      return JSON.stringify(
        entries.map((e) => ({ path: e.path, kind: e.kind, size: e.size, oid: e.oid })),
        null,
        2,
      );
    }

    case "git_fs_merge": {
      const result: MergeResult = await store.merge(
        s("base"),
        s("ours"),
        s("theirs"),
        optS("into") ?? null,
        optS("message") ?? "merge",
      );
      if (result.kind === "clean") {
        return JSON.stringify({ ok: true, tree: result.treeOid, commit: result.commitOid });
      }
      throw new Error(`merge conflicts:\n${JSON.stringify(result.conflicts, null, 2)}`);
    }

    case "git_fs_diff":
      return store.diff(s("ref_a"), s("ref_b"));

    case "git_fs_log": {
      const entries = await store.log(s("ref"), Number(a["count"] ?? 10));
      return JSON.stringify(entries, null, 2);
    }

    case "git_fs_checkout":
      await store.checkout(s("ref"), s("dest"));
      return `Checked out '${s("ref")}' → ${s("dest")}`;

    default:
      throw new Error(`unknown tool: ${name}`);
  }
}

function toolSchemas(): unknown {
  // Verbatim copy of git-fs-mcp tool_list() so existing skill prompts keep working.
  return [
    {
      name: "git_fs_init",
      description: "Initialize a bare git repo at the path in GITFS_REPO env var. Call once before anything else.",
      inputSchema: { type: "object", properties: {}, required: [] },
    },
    {
      name: "git_fs_branch_create",
      description: "Create a branch. Without `from`, creates an empty branch. With `from`, branches off that ref.",
      inputSchema: {
        type: "object",
        properties: {
          name: { type: "string", description: "New branch name" },
          from: { type: "string", description: "Base ref to branch from (optional)" },
        },
        required: ["name"],
      },
    },
    {
      name: "git_fs_branch_list",
      description: "List all branches in the repo.",
      inputSchema: { type: "object", properties: {}, required: [] },
    },
    {
      name: "git_fs_branch_delete",
      description: "Delete a branch.",
      inputSchema: {
        type: "object",
        properties: { name: { type: "string", description: "Branch to delete" } },
        required: ["name"],
      },
    },
    {
      name: "git_fs_write",
      description:
        "Write text content to a file in a branch. Creates a commit. Intermediate directories are created automatically.",
      inputSchema: {
        type: "object",
        properties: {
          branch: { type: "string", description: "Target branch" },
          path: { type: "string", description: "File path, e.g. src/main.rs" },
          content: { type: "string", description: "Full file content" },
          message: { type: "string", description: "Commit message (default: 'write')" },
        },
        required: ["branch", "path", "content"],
      },
    },
    {
      name: "git_fs_read",
      description:
        "Read file content from a ref. Use start_line/end_line (1-indexed, inclusive) to read a slice instead of the full file.",
      inputSchema: {
        type: "object",
        properties: {
          ref: { type: "string", description: "Git ref to read from" },
          path: { type: "string", description: "File path" },
          start_line: { type: "integer", description: "First line to return (1-indexed, inclusive)" },
          end_line: { type: "integer", description: "Last line to return (1-indexed, inclusive)" },
        },
        required: ["ref", "path"],
      },
    },
    {
      name: "git_fs_rm",
      description: "Remove a file from a branch. Creates a commit.",
      inputSchema: {
        type: "object",
        properties: {
          branch: { type: "string", description: "Target branch" },
          path: { type: "string", description: "File path to remove" },
          message: { type: "string", description: "Commit message (optional)" },
        },
        required: ["branch", "path"],
      },
    },
    {
      name: "git_fs_ls",
      description: "List files in a ref. Use recursive=true to walk all subdirectories.",
      inputSchema: {
        type: "object",
        properties: {
          ref: { type: "string", description: "Git ref" },
          path: { type: "string", description: "Subdirectory to list (optional, default: root)" },
          recursive: { type: "boolean", description: "Walk subdirectories (default: false)" },
        },
        required: ["ref"],
      },
    },
    {
      name: "git_fs_merge",
      description:
        "3-way merge two branches. On conflict, returns conflict details as error. On success with `into`, commits the merge.",
      inputSchema: {
        type: "object",
        properties: {
          ours: { type: "string", description: "Our branch" },
          theirs: { type: "string", description: "Their branch" },
          base: { type: "string", description: "Common ancestor ref" },
          into: { type: "string", description: "Commit merge result to this branch (optional)" },
          message: { type: "string", description: "Merge commit message (default: 'merge')" },
        },
        required: ["ours", "theirs", "base"],
      },
    },
    {
      name: "git_fs_diff",
      description: "Unified diff between two refs.",
      inputSchema: {
        type: "object",
        properties: {
          ref_a: { type: "string", description: "First ref" },
          ref_b: { type: "string", description: "Second ref" },
        },
        required: ["ref_a", "ref_b"],
      },
    },
    {
      name: "git_fs_log",
      description: "Commit log for a ref.",
      inputSchema: {
        type: "object",
        properties: {
          ref: { type: "string", description: "Git ref" },
          count: { type: "integer", description: "Max commits to return (default: 10)" },
        },
        required: ["ref"],
      },
    },
    {
      name: "git_fs_patch",
      description:
        "Replace a line range in a file and commit. Equivalent to Edit but writes directly to git history. Lines are 1-indexed, inclusive.",
      inputSchema: {
        type: "object",
        properties: {
          branch: { type: "string", description: "Target branch" },
          path: { type: "string", description: "File path" },
          start_line: { type: "integer", description: "First line to replace (1-indexed, inclusive)" },
          end_line: { type: "integer", description: "Last line to replace (1-indexed, inclusive)" },
          content: { type: "string", description: "Replacement content for the specified line range" },
          message: { type: "string", description: "Commit message (default: 'patch')" },
        },
        required: ["branch", "path", "start_line", "end_line", "content"],
      },
    },
    {
      name: "git_fs_replace",
      description:
        "Replace an exact string in a file and commit. old_str must match exactly once — include surrounding context to make it unique. Immune to line-number drift. Prefer over git_fs_patch for most edits.",
      inputSchema: {
        type: "object",
        properties: {
          branch: { type: "string", description: "Target branch" },
          path: { type: "string", description: "File path" },
          old_str: { type: "string", description: "Exact string to replace (must match exactly once)" },
          new_str: { type: "string", description: "Replacement string" },
          message: { type: "string", description: "Commit message (default: 'replace')" },
        },
        required: ["branch", "path", "old_str", "new_str"],
      },
    },
    {
      name: "git_fs_checkout",
      description: "Materialize a ref to disk. Final extraction step — writes all files to dest directory.",
      inputSchema: {
        type: "object",
        properties: {
          ref: { type: "string", description: "Git ref to materialize" },
          dest: { type: "string", description: "Destination directory path" },
        },
        required: ["ref", "dest"],
      },
    },
  ];
}

function main() {
  const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });

  rl.on("line", async (line) => {
    const trimmed = line.trim();
    if (trimmed.length === 0) return;

    let msg: JsonRpcMessage;
    try {
      msg = JSON.parse(trimmed);
    } catch {
      return;
    }
    if (msg.id === undefined || msg.id === null) return; // notifications

    const method = msg.method ?? "";
    const params = (msg.params ?? {}) as Record<string, unknown>;

    try {
      const result = await dispatch(method, params);
      process.stdout.write(ok(msg.id, result) + "\n");
    } catch (e) {
      process.stdout.write(err(msg.id, -32603, e instanceof Error ? e.message : String(e)) + "\n");
    }
  });
}

main();
