// src/hooks.mjs — Claude Code lifecycle hooks (§7).
// session-start seeds an empty branch; post-edit/post-write capture deltas;
// read enforces read-your-writes; stop materializes touched files to disk.
// git-fs never hard-fails a session: missing `git` ⇒ no-op (§3).

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import * as store from "./store.mjs";

function readStdin() {
  try { return fs.readFileSync(0, "utf8"); } catch { return ""; }
}

function gitAvailable() {
  try { execFileSync("git", ["--version"], { stdio: "ignore" }); return true; }
  catch { return false; }
}

let warned = false;
function noopOnce(msg) {
  if (!warned) { process.stderr.write(`[git-fs] ${msg}\n`); warned = true; }
}

const DISK = () => process.env.GIT_FS_DISK || process.cwd();

// extract the target file path from a Claude Code hook payload
function filePathOf(payload) {
  const ti = payload.tool_input || payload.toolInput || {};
  return ti.file_path || ti.filePath || ti.path || payload.file_path || null;
}

function relPath(abs) {
  if (!abs) return null;
  const r = path.relative(DISK(), path.resolve(DISK(), abs));
  if (r.startsWith("..") || path.isAbsolute(r)) return null; // outside the overlay
  return r.split(path.sep).join("/");
}

function agentBaseOid(branch) {
  try {
    const txt = store.readText(branch, ".agent");
    const m = txt.match(/^base:(.+)$/m);
    if (m) return m[1].trim();
  } catch {}
  return store.rootCommit(branch);
}

export function runHook(action) {
  const payload = (() => { try { return JSON.parse(readStdin() || "{}"); } catch { return {}; } })();
  const sid = payload.session_id || payload.sessionId;
  if (!sid) return;
  if (!gitAvailable()) { noopOnce("git not found — git-fs is a no-op this session"); return; }

  const branch = `gitfs/${sid}`;

  try {
    switch (action) {
      case "session-start": {
        store.ensureStore();
        const seed = store.resolveTip(branch) || store.branchCreate(branch, null); // EMPTY seed
        const model = payload.model || payload.model_id || "unknown";
        const agent = `session:${sid}\nmodel:${model}\nbase:${seed}\ntrack:main\n`;
        store.writeFile(branch, ".agent", Buffer.from(agent, "utf8"), "init .agent");
        process.stderr.write(
          `[git-fs] session ${sid} on ${branch}: reads fall back to disk; only touched files are tracked.\n`,
        );
        break;
      }
      case "post-edit":
      case "post-write": {
        const rel = relPath(filePathOf(payload));
        if (!rel) return;
        const abs = path.join(DISK(), rel);
        let bytes;
        try { bytes = fs.readFileSync(abs); } catch { return; } // file gone ⇒ nothing to capture
        store.writeFile(branch, rel, bytes, `${action === "post-edit" ? "edit" : "write"} ${rel}`);
        break;
      }
      case "read": {
        const rel = relPath(filePathOf(payload));
        if (!rel) return;
        if (!store.isTracked(branch, rel)) return; // overlay default: leave disk as-is
        let branchBytes, diskBytes;
        try { branchBytes = store.readBlobFromTree(branch, rel); } catch { return; }
        try { diskBytes = fs.readFileSync(path.join(DISK(), rel)); } catch { diskBytes = null; }
        if (diskBytes && Buffer.compare(branchBytes, diskBytes) === 0) return; // already in sync
        const dest = path.join(DISK(), rel);
        fs.mkdirSync(path.dirname(dest), { recursive: true });
        fs.writeFileSync(dest, branchBytes); // read-your-writes
        break;
      }
      case "stop": {
        if (!store.resolveTip(branch)) return;
        store.materialize(branch, agentBaseOid(branch));
        break;
      }
      default:
        return;
    }
  } catch (e) {
    // never hard-fail the session
    process.stderr.write(`[git-fs] hook ${action} error: ${String(e && e.message || e)}\n`);
  }
}
