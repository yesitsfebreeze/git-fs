/**
 * Store — git object operations.
 *
 * Mirrors the surface of Rust crate `git-fs/src/store.rs` 1:1 so MCP/CLI
 * code can be ported without semantic drift. Backed by isomorphic-git
 * against a bare repository.
 */

import * as git from "isomorphic-git";
import fs from "node:fs";
import path from "node:path";
import { Buffer } from "node:buffer";
import { createPatch } from "diff";
import { merge as diff3merge } from "node-diff3";

export interface ListEntry {
  path: string;
  kind: "blob" | "tree";
  size: number;
  oid: string;
}

export interface LogEntry {
  oid: string;
  message: string;
  author: string;
  time: number;
}

export interface ConflictEntry {
  path: string;
  ancestor: string | null;
  ours: string | null;
  theirs: string | null;
}

export type MergeResult =
  | { kind: "clean"; treeOid: string; commitOid: string | null }
  | { kind: "conflicts"; conflicts: ConflictEntry[] };

interface TreeEntry {
  mode: string;
  path: string;
  oid: string;
  type: "blob" | "tree" | "commit";
}

const MODE_BLOB = "100644";
const MODE_TREE = "040000";

const AUTHOR = { name: "git-fs", email: "git-fs@local" } as const;

const decoder = new TextDecoder("utf-8", { fatal: true });

export class Store {
  private readonly gitdir: string;

  private constructor(gitdir: string) {
    this.gitdir = gitdir;
  }

  static async open(repo: string): Promise<Store> {
    if (!fs.existsSync(path.join(repo, "HEAD"))) {
      throw new Error(`Cannot open repo '${repo}'. Run 'git-fs init' first.`);
    }
    return new Store(repo);
  }

  static async init(repo: string): Promise<Store> {
    await fs.promises.mkdir(repo, { recursive: true });
    await git.init({ fs, dir: repo, gitdir: repo, bare: true });
    return new Store(repo);
  }

  // ── helpers ───────────────────────────────────────────────────────────────

  private static branchRef(branch: string): string {
    return branch.startsWith("refs/") ? branch : `refs/heads/${branch}`;
  }

  private static withSessionTrailer(msg: string, branch: string): string {
    const session = branch.startsWith("agent/") ? branch.slice("agent/".length) : null;
    if (!session) return msg;
    if (msg.includes("Session-Id:")) return msg;
    const trimmed = msg.replace(/\n+$/, "");
    if (trimmed.length === 0) return `Session-Id: ${session}\n`;
    return `${trimmed}\n\nSession-Id: ${session}\n`;
  }

  private now() {
    return {
      ...AUTHOR,
      timestamp: Math.floor(Date.now() / 1000),
      timezoneOffset: new Date().getTimezoneOffset(),
    };
  }

  private async resolveCommitOid(ref: string): Promise<string> {
    try {
      return await git.resolveRef({ fs, gitdir: this.gitdir, ref });
    } catch {
      // Allow raw OIDs / short OIDs / qualified refs.
      return await git.expandOid({ fs, gitdir: this.gitdir, oid: ref });
    }
  }

  private async resolveTreeOid(ref: string): Promise<string> {
    const commitOid = await this.resolveCommitOid(ref);
    const { commit } = await git.readCommit({ fs, gitdir: this.gitdir, oid: commitOid });
    return commit.tree;
  }

  private async tryReadRefCommit(refname: string): Promise<string | null> {
    try {
      return await git.resolveRef({ fs, gitdir: this.gitdir, ref: refname });
    } catch {
      return null;
    }
  }

  // ── branch ────────────────────────────────────────────────────────────────

  async branchCreate(name: string, from?: string): Promise<void> {
    if (from) {
      const oid = await this.resolveCommitOid(from);
      await git.writeRef({
        fs,
        gitdir: this.gitdir,
        ref: Store.branchRef(name),
        value: oid,
        force: false,
      });
      return;
    }
    // Empty branch: synthesize a parentless commit pointing at an empty tree.
    const treeOid = await git.writeTree({ fs, gitdir: this.gitdir, tree: [] });
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: "init",
        tree: treeOid,
        parent: [],
        author: sig,
        committer: sig,
      },
    });
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: Store.branchRef(name),
      value: commitOid,
      force: false,
    });
  }

  async branchList(): Promise<string[]> {
    return git.listBranches({ fs, gitdir: this.gitdir });
  }

  async branchDelete(name: string): Promise<void> {
    await git.deleteBranch({ fs, gitdir: this.gitdir, ref: name });
  }

  // ── file ops ──────────────────────────────────────────────────────────────

  async writeFile(branch: string, filepath: string, content: Uint8Array, msg: string): Promise<string> {
    const refname = Store.branchRef(branch);
    const blobOid = await git.writeBlob({ fs, gitdir: this.gitdir, blob: content });

    const parentOid = await this.tryReadRefCommit(refname);
    const baseTreeOid = parentOid
      ? (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree
      : null;

    const newTreeOid = await this.treeInsert(baseTreeOid, filepath, blobOid);
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: Store.withSessionTrailer(msg, branch),
        tree: newTreeOid,
        parent: parentOid ? [parentOid] : [],
        author: sig,
        committer: sig,
      },
    });
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: refname,
      value: commitOid,
      force: true,
    });
    return commitOid;
  }

  async readFile(ref: string, filepath: string): Promise<Uint8Array> {
    const treeOid = await this.resolveTreeOid(ref);
    const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: treeOid, filepath });
    return blob;
  }

  private async readText(ref: string, filepath: string): Promise<string> {
    try {
      return decoder.decode(await this.readFile(ref, filepath));
    } catch {
      throw new Error(`'${filepath}' is not valid UTF-8`);
    }
  }

  async readFileLines(ref: string, filepath: string, start?: number, end?: number): Promise<string> {
    const text = await this.readText(ref, filepath);
    const lines = text.split("\n");
    // text.split keeps a trailing empty string when file ends with \n; drop it so
    // line indexing matches Rust's `text.lines()` which excludes the empty tail.
    if (lines.length > 0 && lines[lines.length - 1] === "" && text.endsWith("\n")) {
      lines.pop();
    }
    const total = lines.length;
    const s = Math.max(0, (start ?? 1) - 1);
    const e = Math.min(total, end ?? total);
    if (s >= e && total > 0) {
      throw new Error(`start_line ${start} >= end_line ${end}`);
    }
    return lines.slice(s, e).join("\n");
  }

  async patchFile(
    branch: string,
    filepath: string,
    startLine: number,
    endLine: number,
    newContent: string,
    msg: string,
  ): Promise<string> {
    const text = await this.readText(branch, filepath);
    const hadTrailingNewline = text.endsWith("\n");
    const lines = text.split("\n");
    if (lines.length > 0 && lines[lines.length - 1] === "" && hadTrailingNewline) {
      lines.pop();
    }
    const s = Math.max(0, startLine - 1);
    const e = Math.min(lines.length, endLine);
    if (s > e) throw new Error(`start_line ${startLine} > end_line ${endLine}`);
    const replacement = newContent.split("\n");
    if (replacement.length > 0 && replacement[replacement.length - 1] === "" && newContent.endsWith("\n")) {
      replacement.pop();
    }
    lines.splice(s, e - s, ...replacement);
    let result = lines.join("\n");
    if (hadTrailingNewline) result += "\n";
    return this.writeFile(branch, filepath, Buffer.from(result, "utf-8"), msg);
  }

  async replaceFile(
    branch: string,
    filepath: string,
    oldStr: string,
    newStr: string,
    msg: string,
  ): Promise<string> {
    const text = await this.readText(branch, filepath);
    const idx = text.indexOf(oldStr);
    if (idx < 0) throw new Error(`old_str not found in '${filepath}'`);
    if (text.indexOf(oldStr, idx + oldStr.length) >= 0) {
      // count remaining matches for parity with Rust's wording
      let count = 1;
      let from = idx + oldStr.length;
      while (true) {
        const next = text.indexOf(oldStr, from);
        if (next < 0) break;
        count++;
        from = next + oldStr.length;
      }
      throw new Error(
        `old_str matches ${count} locations in '${filepath}'; make it unique by including more context`,
      );
    }
    const result = text.slice(0, idx) + newStr + text.slice(idx + oldStr.length);
    return this.writeFile(branch, filepath, Buffer.from(result, "utf-8"), msg);
  }

  async removeFile(branch: string, filepath: string, msg: string): Promise<string> {
    const refname = Store.branchRef(branch);
    const parentOid = await this.tryReadRefCommit(refname);
    if (!parentOid) throw new Error(`branch '${branch}' has no commits`);
    const baseTreeOid = (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree;
    const newTreeOid = await this.treeRemove(baseTreeOid, filepath);
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: Store.withSessionTrailer(msg, branch),
        tree: newTreeOid,
        parent: [parentOid],
        author: sig,
        committer: sig,
      },
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: refname, value: commitOid, force: true });
    return commitOid;
  }

  async listFiles(ref: string, prefix: string, recursive: boolean): Promise<ListEntry[]> {
    const rootTreeOid = await this.resolveTreeOid(ref);
    let scopeTreeOid = rootTreeOid;
    if (prefix.length > 0) {
      const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: rootTreeOid, filepath: prefix });
      // readTree with filepath: returns the tree at that path
      scopeTreeOid = await git.writeTree({ fs, gitdir: this.gitdir, tree });
    }
    const out: ListEntry[] = [];
    await this.walkTree(scopeTreeOid, prefix, recursive, out);
    return out;
  }

  private async walkTree(treeOid: string, prefix: string, recursive: boolean, out: ListEntry[]): Promise<void> {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: treeOid });
    for (const entry of tree) {
      const full = prefix.length === 0 ? entry.path : `${prefix}/${entry.path}`;
      if (entry.type === "blob") {
        const { object } = await git.readObject({ fs, gitdir: this.gitdir, oid: entry.oid });
        const size = (object as Uint8Array).byteLength;
        out.push({ path: full, kind: "blob", size, oid: entry.oid });
      } else if (entry.type === "tree") {
        if (recursive) {
          await this.walkTree(entry.oid, full, recursive, out);
        } else {
          out.push({ path: full, kind: "tree", size: 0, oid: entry.oid });
        }
      }
    }
  }

  // ── merge / log / diff / checkout ─────────────────────────────────────────

  /**
   * 3-way merge of `base/ours/theirs` commit refs. When a path has changed on
   * both sides and the contents differ from the base, a conflict is reported.
   * Otherwise the non-base side wins. Empty `theirs` deletes; empty `ours`
   * keeps theirs; both sides identical → no-op.
   */
  async merge(
    baseRef: string,
    oursRef: string,
    theirsRef: string,
    into: string | null,
    msg: string,
  ): Promise<MergeResult> {
    const baseCommit = await this.resolveCommitOid(baseRef);
    const oursCommit = await this.resolveCommitOid(oursRef);
    const theirsCommit = await this.resolveCommitOid(theirsRef);
    const baseTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: baseCommit })).commit.tree;
    const ourTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: oursCommit })).commit.tree;
    const theirTree = (await git.readCommit({ fs, gitdir: this.gitdir, oid: theirsCommit })).commit.tree;

    const merged = await this.mergeTrees(baseTree, ourTree, theirTree);
    if (merged.conflicts.length > 0) {
      return { kind: "conflicts", conflicts: merged.conflicts };
    }
    if (!into) return { kind: "clean", treeOid: merged.treeOid, commitOid: null };

    const targetRef = Store.branchRef(into);
    const targetTip = await this.tryReadRefCommit(targetRef);
    // target's current tip must be first parent for ref update to land.
    const parents =
      targetTip === theirsCommit ? [theirsCommit, oursCommit] : [oursCommit, theirsCommit];
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: Store.withSessionTrailer(msg, oursRef),
        tree: merged.treeOid,
        parent: parents,
        author: sig,
        committer: sig,
      },
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: targetRef, value: commitOid, force: true });
    return { kind: "clean", treeOid: merged.treeOid, commitOid };
  }

  private async mergeTrees(
    baseTreeOid: string,
    ourTreeOid: string,
    theirTreeOid: string,
  ): Promise<{ treeOid: string; conflicts: ConflictEntry[] }> {
    const conflicts: ConflictEntry[] = [];
    const treeOid = await this.mergeTreeRec(baseTreeOid, ourTreeOid, theirTreeOid, "", conflicts);
    return { treeOid, conflicts };
  }

  private async mergeTreeRec(
    baseTreeOid: string | null,
    ourTreeOid: string | null,
    theirTreeOid: string | null,
    pathPrefix: string,
    conflicts: ConflictEntry[],
  ): Promise<string> {
    const base = baseTreeOid ? await this.readTreeMap(baseTreeOid) : new Map<string, TreeEntry>();
    const ours = ourTreeOid ? await this.readTreeMap(ourTreeOid) : new Map<string, TreeEntry>();
    const theirs = theirTreeOid ? await this.readTreeMap(theirTreeOid) : new Map<string, TreeEntry>();

    const names = new Set<string>([...base.keys(), ...ours.keys(), ...theirs.keys()]);
    const out: TreeEntry[] = [];

    for (const name of names) {
      const b = base.get(name);
      const o = ours.get(name);
      const t = theirs.get(name);
      const full = pathPrefix.length === 0 ? name : `${pathPrefix}/${name}`;

      // Type-uniform 3-way: blob/blob/blob is the common case. Trees recurse.
      const types = new Set<string>(
        [b, o, t].filter((x): x is TreeEntry => !!x).map((x) => x.type),
      );

      if (types.size > 1) {
        conflicts.push({
          path: full,
          ancestor: await this.maybeBlobText(b),
          ours: await this.maybeBlobText(o),
          theirs: await this.maybeBlobText(t),
        });
        if (o) out.push(o);
        continue;
      }

      const onlyType = [...types][0];
      if (onlyType === "tree") {
        const subOid = await this.mergeTreeRec(
          b?.oid ?? null,
          o?.oid ?? null,
          t?.oid ?? null,
          full,
          conflicts,
        );
        // Skip if recursion produced an empty tree on both sides (shouldn't normally).
        out.push({ mode: MODE_TREE, path: name, oid: subOid, type: "tree" });
        continue;
      }

      // blob / blob / blob (or some sides missing)
      const oOid = o?.oid;
      const tOid = t?.oid;
      const bOid = b?.oid;

      if (oOid === tOid) {
        // Both sides agree (incl. both deleted).
        if (o) out.push(o);
        continue;
      }
      if (oOid === bOid) {
        // Ours unchanged → take theirs (incl. deletion).
        if (t) out.push(t);
        continue;
      }
      if (tOid === bOid) {
        // Theirs unchanged → keep ours (incl. deletion).
        if (o) out.push(o);
        continue;
      }
      // Both sides diverged from base. Try 3-way line merge before declaring
      // conflict (parity with libgit2's merge_trees auto-merge).
      const lineMerge = await this.tryLineMerge(b, o, t);
      if (lineMerge.clean) {
        const mergedOid = await git.writeBlob({
          fs,
          gitdir: this.gitdir,
          blob: Buffer.from(lineMerge.text, "utf-8"),
        });
        out.push({ mode: MODE_BLOB, path: name, oid: mergedOid, type: "blob" });
        continue;
      }
      conflicts.push({
        path: full,
        ancestor: await this.maybeBlobText(b),
        ours: await this.maybeBlobText(o),
        theirs: await this.maybeBlobText(t),
      });
      if (o) out.push(o);
    }

    out.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
    return git.writeTree({ fs, gitdir: this.gitdir, tree: out });
  }

  private async readTreeMap(oid: string): Promise<Map<string, TreeEntry>> {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid });
    const m = new Map<string, TreeEntry>();
    for (const e of tree) m.set(e.path, e as TreeEntry);
    return m;
  }

  /**
   * 3-way line merge using diff3. Returns clean=true when no conflicting
   * hunks; false when either side touches the same lines or any side is
   * non-text. Matches libgit2's behavior of auto-merging non-overlapping
   * concurrent changes to the same file.
   */
  private async tryLineMerge(
    base: TreeEntry | undefined,
    ours: TreeEntry | undefined,
    theirs: TreeEntry | undefined,
  ): Promise<{ clean: true; text: string } | { clean: false }> {
    if (!base || !ours || !theirs) return { clean: false };
    if (base.type !== "blob" || ours.type !== "blob" || theirs.type !== "blob") {
      return { clean: false };
    }
    const baseText = await this.maybeBlobText(base);
    const ourText = await this.maybeBlobText(ours);
    const theirText = await this.maybeBlobText(theirs);
    if (baseText === null || ourText === null || theirText === null) return { clean: false };
    try {
      const merged = diff3merge(ourText, baseText, theirText, { stringSeparator: /\r?\n/ });
      if (merged.conflict) return { clean: false };
      return { clean: true, text: merged.result.join("\n") };
    } catch {
      return { clean: false };
    }
  }

  private async maybeBlobText(entry: TreeEntry | undefined): Promise<string | null> {
    if (!entry || entry.type !== "blob") return null;
    try {
      const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: entry.oid });
      return decoder.decode(blob);
    } catch {
      return null;
    }
  }

  async log(ref: string, count: number): Promise<LogEntry[]> {
    const commits = await git.log({ fs, gitdir: this.gitdir, ref, depth: count });
    return commits.map((c) => ({
      oid: c.oid,
      message: c.commit.message.trim(),
      author: c.commit.author.name,
      time: c.commit.author.timestamp,
    }));
  }

  async diff(refA: string, refB: string): Promise<string> {
    const treeA = await this.resolveTreeOid(refA);
    const treeB = await this.resolveTreeOid(refB);
    const changes = await this.diffTrees(treeA, treeB, "");
    return changes.join("");
  }

  private async diffTrees(treeA: string, treeB: string, pathPrefix: string): Promise<string[]> {
    const a = await this.readTreeMap(treeA);
    const b = await this.readTreeMap(treeB);
    const names = new Set<string>([...a.keys(), ...b.keys()]);
    const out: string[] = [];
    const sorted = [...names].sort();
    for (const name of sorted) {
      const ea = a.get(name);
      const eb = b.get(name);
      const full = pathPrefix.length === 0 ? name : `${pathPrefix}/${name}`;
      if (ea && eb && ea.oid === eb.oid) continue;
      if (ea?.type === "tree" || eb?.type === "tree") {
        out.push(...(await this.diffTrees(ea?.oid ?? "4b825dc642cb6eb9a060e54bf8d69288fbee4904", eb?.oid ?? "4b825dc642cb6eb9a060e54bf8d69288fbee4904", full)));
        continue;
      }
      const aText = ea ? await this.maybeBlobText(ea) : null;
      const bText = eb ? await this.maybeBlobText(eb) : null;
      out.push(unifiedDiff(full, aText, bText));
    }
    return out;
  }

  async checkout(ref: string, dest: string): Promise<void> {
    const treeOid = await this.resolveTreeOid(ref);
    await fs.promises.mkdir(dest, { recursive: true });
    await this.extractTree(treeOid, dest);
  }

  private async extractTree(treeOid: string, dest: string): Promise<void> {
    const { tree } = await git.readTree({ fs, gitdir: this.gitdir, oid: treeOid });
    for (const entry of tree) {
      const p = path.join(dest, entry.path);
      if (entry.type === "blob") {
        const { blob } = await git.readBlob({ fs, gitdir: this.gitdir, oid: entry.oid });
        await fs.promises.writeFile(p, blob);
      } else if (entry.type === "tree") {
        await fs.promises.mkdir(p, { recursive: true });
        await this.extractTree(entry.oid, p);
      }
    }
  }

  // ── tree helpers ──────────────────────────────────────────────────────────

  private async treeInsert(baseTreeOid: string | null, filepath: string, blobOid: string): Promise<string> {
    const idx = filepath.indexOf("/");
    const head = idx < 0 ? filepath : filepath.slice(0, idx);
    const tail = idx < 0 ? null : filepath.slice(idx + 1);
    const base = baseTreeOid ? await this.readTreeMap(baseTreeOid) : new Map<string, TreeEntry>();

    if (tail === null) {
      base.set(head, { mode: MODE_BLOB, path: head, oid: blobOid, type: "blob" });
    } else {
      const sub = base.get(head);
      const subBaseOid = sub?.type === "tree" ? sub.oid : null;
      const newSubOid = await this.treeInsert(subBaseOid, tail, blobOid);
      base.set(head, { mode: MODE_TREE, path: head, oid: newSubOid, type: "tree" });
    }
    const arr = [...base.values()].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
    return git.writeTree({ fs, gitdir: this.gitdir, tree: arr });
  }

  private async treeRemove(baseTreeOid: string, filepath: string): Promise<string> {
    const idx = filepath.indexOf("/");
    const head = idx < 0 ? filepath : filepath.slice(0, idx);
    const tail = idx < 0 ? null : filepath.slice(idx + 1);
    const base = await this.readTreeMap(baseTreeOid);

    if (tail === null) {
      if (!base.has(head)) throw new Error(`File '${filepath}' not found`);
      base.delete(head);
    } else {
      const sub = base.get(head);
      if (!sub || sub.type !== "tree") throw new Error(`File '${filepath}' not found`);
      const newSubOid = await this.treeRemove(sub.oid, tail);
      base.set(head, { mode: MODE_TREE, path: head, oid: newSubOid, type: "tree" });
    }
    const arr = [...base.values()].sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
    return git.writeTree({ fs, gitdir: this.gitdir, tree: arr });
  }

  // ── extra helpers needed by hooks ─────────────────────────────────────────

  async resolveCommit(ref: string): Promise<string> {
    return this.resolveCommitOid(ref);
  }

  /** Force-set a ref's value (bootstrap main from agent etc). */
  async writeBranchRef(branch: string, oid: string): Promise<void> {
    await git.writeRef({
      fs,
      gitdir: this.gitdir,
      ref: Store.branchRef(branch),
      value: oid,
      force: true,
    });
  }

  /**
   * Remove many paths in one commit. Missing paths are silently skipped.
   * Returns the new commit OID, or null when `paths` is empty / all missing.
   */
  async removePaths(branch: string, paths: string[], msg: string): Promise<string | null> {
    if (paths.length === 0) return null;
    const refname = Store.branchRef(branch);
    const parentOid = await this.tryReadRefCommit(refname);
    if (!parentOid) return null;
    let treeOid = (await git.readCommit({ fs, gitdir: this.gitdir, oid: parentOid })).commit.tree;
    let removed = 0;
    for (const p of paths) {
      try {
        treeOid = await this.treeRemove(treeOid, p);
        removed++;
      } catch {
        // missing path → skip
      }
    }
    if (removed === 0) return null;
    const sig = this.now();
    const commitOid = await git.writeCommit({
      fs,
      gitdir: this.gitdir,
      commit: {
        message: Store.withSessionTrailer(msg, branch),
        tree: treeOid,
        parent: [parentOid],
        author: sig,
        committer: sig,
      },
    });
    await git.writeRef({ fs, gitdir: this.gitdir, ref: refname, value: commitOid, force: true });
    return commitOid;
  }

  /** Set of paths that differ between two refs (add/modify/delete). */
  async changedPaths(fromRef: string, toRef: string): Promise<string[]> {
    const fromTree = await this.resolveTreeOid(fromRef);
    const toTree = await this.resolveTreeOid(toRef);
    const out: string[] = [];
    await this.collectChanged(fromTree, toTree, "", out);
    out.sort();
    return Array.from(new Set(out));
  }

  private async collectChanged(a: string, b: string, prefix: string, out: string[]): Promise<void> {
    if (a === b) return;
    const am = await this.readTreeMap(a);
    const bm = await this.readTreeMap(b);
    const names = new Set<string>([...am.keys(), ...bm.keys()]);
    for (const name of names) {
      const ea = am.get(name);
      const eb = bm.get(name);
      const full = prefix.length === 0 ? name : `${prefix}/${name}`;
      if (ea?.oid === eb?.oid) continue;
      if (ea?.type === "tree" && eb?.type === "tree") {
        await this.collectChanged(ea.oid, eb.oid, full, out);
        continue;
      }
      out.push(full);
    }
  }

  async tipTime(ref: string): Promise<number> {
    const oid = await this.resolveCommitOid(ref);
    const { commit } = await git.readCommit({ fs, gitdir: this.gitdir, oid });
    return commit.author.timestamp;
  }

  async mergeBase(aRef: string, bRef: string): Promise<string | null> {
    try {
      const a = await this.resolveCommitOid(aRef);
      const b = await this.resolveCommitOid(bRef);
      const bases = await git.findMergeBase({ fs, gitdir: this.gitdir, oids: [a, b] });
      return bases[0] ?? null;
    } catch {
      return null;
    }
  }

  async isMergedInto(branch: string, into: string): Promise<boolean> {
    try {
      const a = await this.resolveCommitOid(branch);
      const b = await this.resolveCommitOid(into);
      if (a === b) return true;
      return await git.isDescendent({ fs, gitdir: this.gitdir, oid: b, ancestor: a, depth: -1 });
    } catch {
      return false;
    }
  }

  // expose for trailer test
  static __withSessionTrailerForTest(msg: string, branch: string): string {
    return Store.withSessionTrailer(msg, branch);
  }
}

// ── unified diff ─────────────────────────────────────────────────────────────

function unifiedDiff(filepath: string, a: string | null, b: string | null): string {
  // createPatch emits a leading "Index:" header we don't want; strip it.
  // Empty-string-vs-empty-string yields a no-op patch, which is what we want
  // for trees-changed-but-content-identical edge cases.
  const patch = createPatch(filepath, a ?? "", b ?? "", "", "", { context: 3 });
  return patch.replace(/^Index:.*\n=+\n/, "");
}
