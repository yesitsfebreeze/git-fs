// src/store.mjs — git wrapper + overlay primitives (the core).
// Every function runs the system `git` binary against a dedicated bare repo.
// Zero npm deps. See build spec §5–§8.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

// GITFS = the dedicated bare store; DISK = the working tree we overlay.
// Read dynamically so a process (or test) can retarget via env.
const GITFS = () => process.env.GIT_FS_REPO || ".git-fs";
const DISK = () => process.env.GIT_FS_DISK || process.cwd();

// Pointer file (inside the bare store) naming the branch the active session
// owns. session-start writes it; the MCP server and CLI default to it instead
// of a shared "main", so tool writes and the lifecycle hooks converge on the
// same gitfs/<sid> branch rather than splitting into two parallel timelines.
const CURRENT = () => path.join(GITFS(), "CURRENT");

export function setCurrentBranch(branch) {
  try {
    fs.mkdirSync(GITFS(), { recursive: true });
    fs.writeFileSync(CURRENT(), branch + "\n");
  } catch {}
}

export function currentBranch() {
  try {
    const b = fs.readFileSync(CURRENT(), "utf8").trim();
    return b || null;
  } catch {
    return null;
  }
}

// Stable identity so commit-tree works in clean/CI environments.
const IDENT = {
  GIT_AUTHOR_NAME: "git-fs",
  GIT_AUTHOR_EMAIL: "git-fs@localhost",
  GIT_COMMITTER_NAME: "git-fs",
  GIT_COMMITTER_EMAIL: "git-fs@localhost",
};

// ── thin execFile wrapper ──────────────────────────────────────────────────
// Returns stdout as a Buffer (binary-safe). Throws on nonzero exit; the thrown
// error keeps .status/.stdout/.stderr from execFileSync for callers that parse
// failure output (e.g. merge conflicts).
export function git(args, opts = {}) {
  const env = { ...process.env, ...IDENT };
  if (opts.indexFile) {
    env.GIT_INDEX_FILE = opts.indexFile;
    // index mutations (update-index --force-remove) demand a work tree even
    // though our ops never read its contents; a scratch dir satisfies the guard.
    env.GIT_WORK_TREE = path.join(GITFS(), "tmp");
  }
  return execFileSync("git", ["--git-dir=" + GITFS(), ...args], {
    input: opts.input,
    env,
    maxBuffer: 256 * 1024 * 1024,
  });
}

function gitText(args, opts) {
  return git(args, opts).toString("utf8");
}

// ── store lifecycle ─────────────────────────────────────────────────────────
export function ensureStore() {
  if (!fs.existsSync(path.join(GITFS(), "HEAD"))) {
    execFileSync("git", ["init", "--bare", "--quiet", GITFS()], { env: { ...process.env, ...IDENT } });
  }
}

export function resolveTip(branch) {
  try {
    return gitText(["rev-parse", "--verify", "-q", "refs/heads/" + branch]).trim();
  } catch {
    return null;
  }
}

// Root (parentless) commit of a branch — equals the empty seed.
export function rootCommit(branch) {
  return gitText(["rev-list", "--max-parents=0", branch]).trim().split("\n").pop();
}

// ── scratch index (§6) ──────────────────────────────────────────────────────
function tmpIndexFile() {
  const dir = path.join(GITFS(), "tmp");
  fs.mkdirSync(dir, { recursive: true });
  return path.join(dir, `idx-${process.pid}-${Math.random().toString(36).slice(2)}`);
}

function rmIndex(idx) {
  try { fs.rmSync(idx, { force: true }); } catch {}
}

// tiny mkdir-based per-branch lock to serialize concurrent writers (§6)
function sleepSync(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function withBranchLock(branch, fn) {
  const lockRoot = path.join(GITFS(), "locks");
  fs.mkdirSync(lockRoot, { recursive: true });
  const lock = path.join(lockRoot, encodeURIComponent(branch) + ".lock");
  let acquired = false;
  for (let i = 0; i < 2000; i++) {
    try { fs.mkdirSync(lock); acquired = true; break; }
    catch { sleepSync(5); }
  }
  try { return fn(); }
  finally { if (acquired) { try { fs.rmdirSync(lock); } catch {} } }
}

// ── overlay reads (the choke point, §5) ─────────────────────────────────────
export function readBlobFromTree(ref, p) {
  // throws (nonzero exit) if the path is absent in the tree
  return git(["cat-file", "blob", `${ref}:${p}`]);
}

// Disk-read gate (§5): on a branch-tree miss we fall back to the working tree,
// but only for paths the MAIN git already tracks (a git-tree membership query —
// no .gitignore reimplementation) or that .gitfsallow explicitly re-permits.
// Keeps ignored/untracked content (secrets, build junk, deps) out of the
// publishable store. Blocked reads surface as plain NotFound.
function mainGitTracks(p) {
  try {
    execFileSync("git", ["-C", DISK(), "ls-files", "--error-unmatch", "-z", "--", p],
      { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function insideMainGit() {
  try {
    execFileSync("git", ["-C", DISK(), "rev-parse", "--is-inside-work-tree"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function loadAllow() {
  try {
    const txt = fs.readFileSync(path.join(DISK(), ".gitfsallow"), "utf8");
    return txt.split("\n").map((s) => s.trim()).filter((s) => s && !s.startsWith("#"));
  } catch {
    return [];
  }
}

export function diskReadAllowed(p) {
  if (mainGitTracks(p)) return true;                                // in the main git tree
  if (loadAllow().some((pat) => matchPattern(pat, p))) return true; // explicit exception
  return !insideMainGit();                                          // no main repo → nothing to gate
}

export function readFile(ref, p) {
  try {
    return readBlobFromTree(ref, p); // tracked → branch wins (read-your-writes)
  } catch {
    if (!diskReadAllowed(p)) {
      const e = new Error(`Not found: ${p}`);
      e.code = "NotFound";
      throw e;
    }
    try {
      return fs.readFileSync(path.join(DISK(), p)); // untouched → working tree
    } catch {
      const e = new Error(`Not found: ${p}`);
      e.code = "NotFound";
      throw e;
    }
  }
}

export function readText(ref, p) {
  const buf = readFile(ref, p);
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(buf);
  } catch {
    throw new Error(`${p} is not valid UTF-8`);
  }
}

export function readFileLines(ref, p, start, end) {
  const lines = readText(ref, p).split("\n");
  const s = Math.max(1, start);
  const e = end == null ? lines.length : end;
  return lines.slice(s - 1, e).join("\n");
}

// ── commit trailer ──────────────────────────────────────────────────────────
export function trailer(msg, branch) {
  const m = msg || "";
  if (!branch.startsWith("gitfs/")) return m;
  const sid = branch.slice("gitfs/".length);
  if (m.includes(`Session-Id: ${sid}`)) return m; // idempotent
  return `${m}\n\nSession-Id: ${sid}`;
}

// ── writes: capture into the branch ─────────────────────────────────────────
export function writeFile(branch, p, bytes, msg) {
  return withBranchLock(branch, () => {
    const blob = gitText(["hash-object", "-w", "--stdin"], { input: bytes }).trim();
    const parent = resolveTip(branch);
    const idx = tmpIndexFile();
    try {
      if (parent) git(["read-tree", parent], { indexFile: idx });
      git(["update-index", "--add", "--cacheinfo", `100644,${blob},${p}`], { indexFile: idx });
      const tree = gitText(["write-tree"], { indexFile: idx }).trim();
      const args = ["commit-tree", tree];
      if (parent) args.push("-p", parent);
      args.push("-m", trailer(msg, branch));
      const commit = gitText(args).trim();
      git(["update-ref", "refs/heads/" + branch, commit]);
      return commit;
    } finally {
      rmIndex(idx);
    }
  });
}

export function replaceFile(branch, p, oldStr, newStr, msg) {
  const text = readText(branch, p); // disk-fallback applies
  const parts = text.split(oldStr);
  const count = parts.length - 1;
  if (count !== 1) {
    throw new Error(`Expected exactly one occurrence of the search string in ${p}, found ${count}`);
  }
  return writeFile(branch, p, Buffer.from(parts.join(newStr), "utf8"), msg || `replace ${p}`);
}

// Splice lines [start,end] (1-based, inclusive) with `content`, preserving
// trailing-newline semantics.
export function patchFile(branch, p, start, end, content, msg) {
  const text = readText(branch, p);
  const trailing = text.endsWith("\n");
  const body = trailing ? text.slice(0, -1) : text;
  const lines = body.length === 0 ? [] : body.split("\n");

  const cTrailing = content.endsWith("\n");
  const cBody = cTrailing ? content.slice(0, -1) : content;
  const cLines = content.length === 0 ? [] : cBody.split("\n");

  const s = Math.max(1, start);
  const e = Math.min(lines.length, end);
  const count = Math.max(0, e - s + 1);
  lines.splice(s - 1, count, ...cLines);

  let out = lines.join("\n");
  if (trailing && out.length) out += "\n";
  return writeFile(branch, p, Buffer.from(out, "utf8"), msg || `patch ${p}`);
}

export function removeFile(branch, p, msg) {
  return withBranchLock(branch, () => {
    const parent = resolveTip(branch);
    const idx = tmpIndexFile();
    try {
      if (parent) git(["read-tree", parent], { indexFile: idx });
      git(["update-index", "--force-remove", p], { indexFile: idx });
      const tree = gitText(["write-tree"], { indexFile: idx }).trim();
      const args = ["commit-tree", tree];
      if (parent) args.push("-p", parent);
      args.push("-m", trailer(msg || `rm ${p}`, branch));
      const commit = gitText(args).trim();
      git(["update-ref", "refs/heads/" + branch, commit]);
      return commit;
    } finally {
      rmIndex(idx);
    }
  });
}

export function isTracked(branch, p) {
  try { readBlobFromTree(branch, p); return true; } catch { return false; }
}

// Tombstone an untracked (disk-only) path so materialize unlinks it (§8).
export function tombstone(branch, p, msg) {
  let existing = "";
  try { existing = readText(branch, ".git-fs/session/tombstones"); } catch {}
  const set = new Set(existing.split("\n").map((s) => s.trim()).filter(Boolean));
  set.add(p);
  return writeFile(
    branch,
    ".git-fs/session/tombstones",
    Buffer.from([...set].join("\n") + "\n", "utf8"),
    msg || `tombstone ${p}`,
  );
}

// rm dispatch: remove from the branch tree when tracked, and always tombstone
// so materialize unlinks any disk copy regardless of base (empty seed yields no
// D-status for a capture-then-delete; the tombstone carries the intent). §8
export function rm(branch, p, msg) {
  if (isTracked(branch, p)) removeFile(branch, p, msg);
  return tombstone(branch, p, msg);
}

// ── queries ─────────────────────────────────────────────────────────────────
export function listFiles(ref, prefix, recursive) {
  const args = ["ls-tree"];
  if (recursive) args.push("-r");
  args.push("--name-only", ref);
  if (prefix) args.push(prefix);
  try {
    return gitText(args).split("\n").filter(Boolean);
  } catch {
    return [];
  }
}

export function diffNameStatus(base, branch) {
  const out = gitText(["diff", "--name-status", base, branch]);
  return out.split("\n").filter(Boolean).map((line) => {
    const i = line.search(/\s/);
    const status = i === -1 ? line : line.slice(0, i);
    const p = i === -1 ? "" : line.slice(i).trim();
    return [status[0], p];
  });
}

export function log(branch) {
  try { return gitText(["log", "--format=%H %s", branch]); }
  catch { return ""; }
}

export function diff(a, b, p) {
  const args = ["diff", a, b];
  if (p) args.push("--", p);
  return gitText(args);
}

export function branchList() {
  return gitText(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
    .split("\n").filter(Boolean);
}

export function branchCreate(name, from) {
  if (from) {
    git(["update-ref", "refs/heads/" + name, from]);
    return from;
  }
  const idx = tmpIndexFile();
  try {
    git(["read-tree", "--empty"], { indexFile: idx });
    const emptyTree = gitText(["write-tree"], { indexFile: idx }).trim();
    const commit = gitText(["commit-tree", emptyTree, "-m", trailer("init", name)]).trim();
    git(["update-ref", "refs/heads/" + name, commit]);
    return commit;
  } finally {
    rmIndex(idx);
  }
}

export function branchDelete(name) {
  try { git(["update-ref", "-d", "refs/heads/" + name]); return true; }
  catch { return false; }
}

// ── merge via `git merge-tree --write-tree` (git >=2.38) ─────────────────────
function parseMergeConflicts(out) {
  const set = new Set();
  for (const line of out.split("\n")) {
    const m = line.match(/^\d{6} [0-9a-f]+ [123]\t(.+)$/);
    if (m) set.add(m[1]);
  }
  return [...set];
}

export function merge(base, ours, theirs) {
  const args = ["merge-tree", "--write-tree"];
  if (base) args.push("--merge-base=" + base);
  args.push(ours, theirs);
  let out;
  let code = 0;
  try {
    out = gitText(args);
  } catch (e) {
    code = e.status ?? 1;
    out = (e.stdout ? e.stdout.toString("utf8") : "");
  }
  const tree = (out.split("\n")[0] || "").trim();
  if (code === 0) {
    const commit = gitText(["commit-tree", tree, "-p", ours, "-p", theirs, "-m", "merge"]).trim();
    return { clean: true, tree, commit, conflicts: [] };
  }
  return { clean: false, tree, conflicts: parseMergeConflicts(out) };
}

// ── ignore matcher + tombstones (§8) ────────────────────────────────────────
const HARD_IGNORE = [".agent", ".git-fs/", "CONFLICTS.md"];

function matchPattern(pat, p) {
  if (pat.endsWith("/")) return p === pat.slice(0, -1) || p.startsWith(pat); // dir prefix
  if (pat.startsWith("*.")) return p.endsWith(pat.slice(1)); // suffix
  return p === pat; // exact
}

function loadExtraIgnore() {
  try {
    const txt = fs.readFileSync(path.join(GITFS(), "mergeignore"), "utf8");
    return txt.split("\n").map((s) => s.trim()).filter((s) => s && !s.startsWith("#"));
  } catch {
    return [];
  }
}

export function ignored(p) {
  const pats = [...HARD_IGNORE, ...loadExtraIgnore()];
  return pats.some((pat) => matchPattern(pat, p));
}

// The session seed for a branch: the base recorded in .agent, else the branch's
// own root (empty seed). materialize diffs against THIS, so checkout only ever
// touches files the session changed — never a cross-session union of a shared
// branch's whole history.
export function agentBase(branch) {
  try {
    const txt = readText(branch, ".agent");
    const m = txt.match(/^base:(.+)$/m);
    if (m) return m[1].trim();
  } catch {}
  return rootCommit(branch);
}

// committer time (epoch seconds) of the last commit on `branch` that touched p.
function lastCommitTime(branch, p) {
  try {
    const t = gitText(["log", "-1", "--format=%ct", branch, "--", p]).trim();
    return t ? parseInt(t, 10) : null;
  } catch {
    return null;
  }
}

// ── materialize touched files to disk (§7) ──────────────────────────────────
// Returns { conflicts: [path, ...] } — files left untouched because the on-disk
// copy is newer than the blob we would have written (clobber guard).
export function materialize(branch, base, opts = {}) {
  const b = base || agentBase(branch);
  const conflicts = [];
  for (const [status, p] of diffNameStatus(b, branch)) {
    if (!p || ignored(p)) continue;
    const dest = path.join(DISK(), p);
    if (status === "D") {
      try { fs.unlinkSync(dest); } catch {}
      continue;
    }
    // A or M → write the BRANCH tree bytes (no overlay fallback: a real tree
    // miss must surface, not be masked by the disk copy).
    const bytes = readBlobFromTree(branch, p);
    let disk = null;
    try { disk = fs.readFileSync(dest); } catch {}
    if (disk && Buffer.compare(disk, bytes) === 0) continue; // already in sync
    // Clobber guard: refuse to overwrite on-disk content that postdates the
    // blob we are about to write (e.g. a checkout of a stale or foreign branch
    // whose blobs are older than committed work already sitting on disk).
    if (disk && !opts.force) {
      const blobTime = lastCommitTime(branch, p);
      let diskTime = 0;
      try { diskTime = Math.floor(fs.statSync(dest).mtimeMs / 1000); } catch {}
      if (blobTime != null && diskTime > blobTime) {
        conflicts.push(p);
        continue;
      }
    }
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, bytes);
  }
  // honor tombstones for untracked disk-only deletions
  try {
    const tomb = readText(branch, ".git-fs/session/tombstones");
    for (const line of tomb.split("\n").map((s) => s.trim()).filter(Boolean)) {
      if (ignored(line)) continue;
      try { fs.unlinkSync(path.join(DISK(), line)); } catch {}
    }
  } catch {}
  return { conflicts };
}

// Materialize the session branch, scoped to its own seed (never the empty root
// of a shared branch). Pass { force: true } to override the clobber guard.
export function checkout(branch, opts = {}) {
  return materialize(branch, agentBase(branch), opts);
}
