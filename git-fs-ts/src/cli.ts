/**
 * git-fs CLI.
 *
 * Mirrors git-fs/src/main.rs subcommands. Spike scope covers the
 * subcommands exercised by the perf benchmark + manual use; hook
 * subcommands (SessionStart/Stop/etc) are out of scope for the spike
 * and will be ported only if the perf gate passes.
 */

import { Store, MergeResult } from "./store.js";
import { runHook, initProject, runPrune } from "./hooks.js";
import { Buffer } from "node:buffer";
import { stdin as input, stdout as output } from "node:process";

interface Argv {
  repo: string;
  json: boolean;
  positional: string[];
  flags: Record<string, string | boolean>;
}

function parseArgv(argv: string[]): { cmd: string; sub: string | null; rest: Argv } {
  const positional: string[] = [];
  const flags: Record<string, string | boolean> = {};
  let repo = process.env["GIT_FS_REPO"] ?? ".git-fs";
  let json = false;

  let cmd: string | null = null;
  let sub: string | null = null;

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--repo") {
      repo = argv[++i] ?? repo;
    } else if (a.startsWith("--repo=")) {
      repo = a.slice("--repo=".length);
    } else if (a === "--json") {
      json = true;
    } else if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq >= 0) {
        flags[a.slice(2, eq)] = a.slice(eq + 1);
      } else {
        // peek next; if it isn't another flag treat it as value
        const next = argv[i + 1];
        if (next !== undefined && !next.startsWith("--")) {
          flags[a.slice(2)] = next;
          i++;
        } else {
          flags[a.slice(2)] = true;
        }
      }
    } else if (cmd === null) {
      cmd = a;
    } else if ((cmd === "branch" || cmd === "hook") && sub === null) {
      sub = a;
    } else {
      positional.push(a);
    }
  }

  return { cmd: cmd ?? "", sub, rest: { repo, json, positional, flags } };
}

function emit(json: boolean, jsonValue: unknown, plain: string) {
  output.write(json ? JSON.stringify(jsonValue) + "\n" : plain.endsWith("\n") ? plain : plain + "\n");
}

async function readStdin(): Promise<Buffer> {
  const chunks: Buffer[] = [];
  for await (const c of input) chunks.push(c as Buffer);
  return Buffer.concat(chunks);
}

async function main() {
  const { cmd, sub, rest } = parseArgv(process.argv.slice(2));
  const { repo, json, positional: p, flags: f } = rest;

  if (cmd === "" || cmd === "--help" || cmd === "-h") {
    output.write(USAGE);
    return;
  }

  if (cmd === "init") {
    await Store.init(repo);
    emit(json, { ok: true, repo }, `Initialized bare repo: ${repo}`);
    return;
  }

  if (cmd === "init-project") {
    const r = typeof f["repo"] === "string" ? f["repo"] : ".git-fs";
    const cfg = typeof f["mcp-config"] === "string" ? f["mcp-config"] : ".mcp.json";
    await initProject(r, cfg);
    return;
  }

  if (cmd === "hook") {
    process.env["GIT_FS_REPO"] = repo;
    if (sub === null) throw new Error("hook needs a subcommand");
    const allowed = ["session-start", "post-write", "post-edit", "read", "stop"] as const;
    if (!(allowed as readonly string[]).includes(sub)) {
      throw new Error(`unknown hook: ${sub}`);
    }
    await runHook(sub as (typeof allowed)[number]);
    return;
  }

  if (cmd === "prune") {
    await runPrune(repo, {
      merged: f["merged"] === true,
      olderThan: typeof f["older-than"] === "string" ? f["older-than"] : null,
      into: typeof f["into"] === "string" ? f["into"] : "main",
      prefix: typeof f["prefix"] === "string" ? f["prefix"] : "agent/",
      apply: f["apply"] === true,
      json,
    });
    return;
  }

  const store = await Store.open(repo);

  switch (cmd) {
    case "branch": {
      if (sub === "create") {
        const name = req(p, 0, "name");
        await store.branchCreate(name, typeof f["from"] === "string" ? f["from"] : undefined);
        emit(json, { ok: true, branch: name }, `Created branch: ${name}`);
      } else if (sub === "list") {
        const bs = await store.branchList();
        emit(json, bs, bs.join("\n"));
      } else if (sub === "delete") {
        const name = req(p, 0, "name");
        await store.branchDelete(name);
        emit(json, { ok: true, deleted: name }, `Deleted: ${name}`);
      } else {
        throw new Error(`unknown branch subcommand: ${sub}`);
      }
      break;
    }

    case "write": {
      const branch = req(p, 0, "branch");
      const path = req(p, 1, "path");
      const message = typeof f["message"] === "string" ? f["message"] : "write";
      const content = await readStdin();
      const oid = await store.writeFile(branch, path, content, message);
      emit(json, { ok: true, commit: oid, branch, path }, `${oid.slice(0, 8)} ${branch}:${path}`);
      break;
    }

    case "read": {
      const ref = req(p, 0, "ref");
      const path = req(p, 1, "path");
      const bytes = await store.readFile(ref, path);
      output.write(bytes);
      break;
    }

    case "rm": {
      const branch = req(p, 0, "branch");
      const path = req(p, 1, "path");
      const msg = (typeof f["message"] === "string" && f["message"]) || `rm ${path}`;
      const oid = await store.removeFile(branch, path, msg);
      emit(json, { ok: true, commit: oid }, `${oid.slice(0, 8)} rm ${path}`);
      break;
    }

    case "ls": {
      const ref = req(p, 0, "ref");
      const path = p[1] ?? "";
      const recursive = f["recursive"] === true;
      const entries = await store.listFiles(ref, path, recursive);
      if (json) {
        output.write(JSON.stringify(entries) + "\n");
      } else {
        for (const e of entries) {
          if (e.kind === "blob") {
            output.write(`${String(e.size).padStart(8)}B  ${e.oid.slice(0, 8)}  ${e.path}\n`);
          } else {
            output.write(`     dir  ${e.oid.slice(0, 8)}  ${e.path}/\n`);
          }
        }
      }
      break;
    }

    case "merge": {
      const ours = req(p, 0, "ours");
      const theirs = req(p, 1, "theirs");
      const base = String(f["base"] ?? throwIf("--base required"));
      const into = typeof f["into"] === "string" ? f["into"] : null;
      const message = typeof f["message"] === "string" ? f["message"] : "merge";
      const result: MergeResult = await store.merge(base, ours, theirs, into, message);
      if (result.kind === "clean") {
        if (json) {
          output.write(JSON.stringify({ ok: true, tree: result.treeOid, commit: result.commitOid }) + "\n");
        } else if (result.commitOid) {
          output.write(`merged tree ${result.treeOid.slice(0, 8)} → commit ${result.commitOid.slice(0, 8)}\n`);
        } else {
          output.write(`tree ${result.treeOid.slice(0, 8)} (pass --into <branch> to commit)\n`);
        }
      } else {
        if (json) {
          output.write(JSON.stringify({ ok: false, conflicts: result.conflicts }) + "\n");
        } else {
          process.stderr.write(`CONFLICTS (${result.conflicts.length}):\n`);
          for (const c of result.conflicts) process.stderr.write(`  conflict: ${c.path}\n`);
        }
        process.exit(2);
      }
      break;
    }

    case "diff": {
      const refA = req(p, 0, "ref_a");
      const refB = req(p, 1, "ref_b");
      output.write(await store.diff(refA, refB));
      break;
    }

    case "log": {
      const ref = req(p, 0, "ref");
      const count = Number(f["count"] ?? 10);
      const entries = await store.log(ref, count);
      if (json) {
        output.write(JSON.stringify(entries) + "\n");
      } else {
        for (const e of entries) output.write(`${e.oid.slice(0, 8)} ${e.message}\n`);
      }
      break;
    }

    case "checkout": {
      const ref = req(p, 0, "ref");
      const dest = req(p, 1, "dest");
      await store.checkout(ref, dest);
      emit(json, { ok: true, dest }, `Checked out '${ref}' → ${dest}`);
      break;
    }

    case "show": {
      const ref = req(p, 0, "ref");
      const entries = await store.log(ref, 1);
      const e = entries[0];
      if (!e) break;
      if (json) {
        output.write(JSON.stringify(e) + "\n");
      } else {
        output.write(`commit ${e.oid}\nauthor ${e.author}\n\n    ${e.message}\n`);
      }
      break;
    }

    default:
      throw new Error(`unknown command: ${cmd}`);
  }
}

function req(arr: string[], i: number, label: string): string {
  const v = arr[i];
  if (typeof v !== "string") throw new Error(`${label} required`);
  return v;
}

function throwIf(msg: string): never {
  throw new Error(msg);
}

const USAGE = `git-fs — virtual filesystem over git object store

USAGE:
  git-fs [--repo PATH] [--json] <COMMAND>

COMMANDS:
  init                                  Initialize a bare repo
  branch create NAME [--from REF]       Create branch
  branch list                           List branches
  branch delete NAME                    Delete branch
  write BRANCH PATH [--message MSG]     Write stdin to file (auto-commits)
  read REF PATH                         Stream file to stdout
  rm BRANCH PATH [--message MSG]        Remove file
  ls REF [PATH] [--recursive]           List files
  merge OURS THEIRS --base REF [--into BRANCH] [--message MSG]
  diff REF_A REF_B                      Unified diff
  log REF [--count N]                   Commit log
  checkout REF DEST                     Materialize ref to disk
  show REF                              Show tip commit
`;

main().catch((e) => {
  process.stderr.write(`error: ${e instanceof Error ? e.message : String(e)}\n`);
  process.exit(1);
});
