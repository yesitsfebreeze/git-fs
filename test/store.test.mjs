// test/store.test.mjs — node:test, run via `node --test`.
// No build step, no node_modules: only Node + system git required.

import { test, before, beforeEach } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

let store;

function freshSandbox() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "gitfs-test-"));
  const disk = path.join(root, "work");
  const repo = path.join(root, ".git-fs");
  fs.mkdirSync(disk, { recursive: true });
  process.env.GIT_FS_DISK = disk;
  process.env.GIT_FS_REPO = repo;
  store.ensureStore();
  return { root, disk, repo };
}

before(async () => {
  store = await import("../src/store.mjs");
});

const BR = "gitfs/test-session";

test("empty-seed: branch starts with no files", () => {
  freshSandbox();
  const seed = store.branchCreate(BR, null);
  assert.ok(seed, "seed commit oid returned");
  assert.deepEqual(store.listFiles(BR, "", true), [], "no deltas on fresh branch");
});

test("disk-fallback read: untracked file returns disk bytes", () => {
  const { disk } = freshSandbox();
  store.branchCreate(BR, null);
  fs.writeFileSync(path.join(disk, "untracked.txt"), "from disk\n");
  assert.equal(store.readText(BR, "untracked.txt"), "from disk\n");
  // still not a delta — reading does not capture
  assert.deepEqual(store.listFiles(BR, "", true), []);
});

test("read-your-writes: branch blob wins over disk", () => {
  const { disk } = freshSandbox();
  store.branchCreate(BR, null);
  fs.writeFileSync(path.join(disk, "a.txt"), "disk\n");
  store.writeFile(BR, "a.txt", Buffer.from("branch\n"), "edit a");
  assert.equal(store.readText(BR, "a.txt"), "branch\n");
});

test("write/replace/patch round-trip", () => {
  freshSandbox();
  store.branchCreate(BR, null);
  store.writeFile(BR, "f.txt", Buffer.from("one\ntwo\nthree\n"), "init f");

  store.replaceFile(BR, "f.txt", "two", "TWO", "replace");
  assert.equal(store.readText(BR, "f.txt"), "one\nTWO\nthree\n");

  // replace must reject non-unique matches
  store.writeFile(BR, "dup.txt", Buffer.from("x x\n"), "dup");
  assert.throws(() => store.replaceFile(BR, "dup.txt", "x", "y"), /found 2/);

  // patch: splice line 2 (1-based inclusive) -> two lines
  store.patchFile(BR, "f.txt", 2, 2, "TWO-A\nTWO-B\n", "patch");
  assert.equal(store.readText(BR, "f.txt"), "one\nTWO-A\nTWO-B\nthree\n");

  assert.equal(store.readFileLines(BR, "f.txt", 1, 2), "one\nTWO-A");
});

test("materialize touched-only, including a D deletion", () => {
  const { disk } = freshSandbox();
  const seed = store.branchCreate(BR, null);

  // a pre-existing untouched disk file must NOT be rewritten by materialize
  fs.writeFileSync(path.join(disk, "keep.txt"), "untouched\n");

  // touched files
  store.writeFile(BR, "added.txt", Buffer.from("new file\n"), "add");
  fs.writeFileSync(path.join(disk, "del.txt"), "to be deleted\n");
  store.writeFile(BR, "del.txt", Buffer.from("captured\n"), "capture del");
  store.rm(BR, "del.txt", "rm del");

  // only deltas are: added.txt (A) and del.txt (now D vs seed... actually A then D = absent)
  store.materialize(BR, seed);

  assert.equal(fs.readFileSync(path.join(disk, "added.txt"), "utf8"), "new file\n");
  assert.ok(!fs.existsSync(path.join(disk, "del.txt")), "deleted file removed from disk");
  assert.equal(fs.readFileSync(path.join(disk, "keep.txt"), "utf8"), "untouched\n", "untouched file preserved");
});

test("materialize honors D status for a file deleted in branch", () => {
  const { disk } = freshSandbox();
  const seed = store.branchCreate(BR, null);
  // capture then delete; deletion relative to a base where it existed
  const c1 = store.writeFile(BR, "x.txt", Buffer.from("hi\n"), "write x");
  // materialize the write so disk has it
  store.materialize(BR, seed);
  assert.ok(fs.existsSync(path.join(disk, "x.txt")));
  // now delete in branch and materialize relative to c1 (where it was A/M)
  store.removeFile(BR, "x.txt", "rm x");
  store.materialize(BR, c1);
  assert.ok(!fs.existsSync(path.join(disk, "x.txt")), "D status unlinks on disk");
});

test("tombstone: untracked disk file removed on materialize", () => {
  const { disk } = freshSandbox();
  const seed = store.branchCreate(BR, null);
  fs.writeFileSync(path.join(disk, "junk.txt"), "delete me\n");
  store.rm(BR, "junk.txt"); // untracked → tombstone
  store.materialize(BR, seed);
  assert.ok(!fs.existsSync(path.join(disk, "junk.txt")), "tombstoned file unlinked");
});

test("ignore: .agent / .git-fs/ / CONFLICTS.md never materialize", () => {
  const { disk } = freshSandbox();
  const seed = store.branchCreate(BR, null);
  store.writeFile(BR, ".agent", Buffer.from("session:x\n"), "agent");
  store.writeFile(BR, "CONFLICTS.md", Buffer.from("conflict\n"), "conf");
  store.writeFile(BR, "real.txt", Buffer.from("real\n"), "real");
  store.materialize(BR, seed);
  assert.ok(!fs.existsSync(path.join(disk, ".agent")));
  assert.ok(!fs.existsSync(path.join(disk, "CONFLICTS.md")));
  assert.ok(fs.existsSync(path.join(disk, "real.txt")));
});

test("merge clean: non-overlapping changes", () => {
  freshSandbox();
  const seed = store.branchCreate("base", null);
  store.branchCreate("ours", seed);
  store.branchCreate("theirs", seed);
  store.writeFile("ours", "a.txt", Buffer.from("A\n"), "a");
  store.writeFile("theirs", "b.txt", Buffer.from("B\n"), "b");
  const r = store.merge(seed, store.resolveTip("ours"), store.resolveTip("theirs"));
  assert.equal(r.clean, true);
  assert.ok(r.commit);
  // both files present in merged tree
  const files = store.listFiles(r.commit, "", true);
  assert.ok(files.includes("a.txt") && files.includes("b.txt"));
});

test("merge conflict: overlapping edits report conflicted path", () => {
  freshSandbox();
  const seed = store.branchCreate("base", null);
  store.writeFile("base", "c.txt", Buffer.from("base line\n"), "base");
  const baseTip = store.resolveTip("base");
  store.branchCreate("ours", baseTip);
  store.branchCreate("theirs", baseTip);
  store.writeFile("ours", "c.txt", Buffer.from("ours line\n"), "ours");
  store.writeFile("theirs", "c.txt", Buffer.from("theirs line\n"), "theirs");
  const r = store.merge(baseTip, store.resolveTip("ours"), store.resolveTip("theirs"));
  assert.equal(r.clean, false);
  assert.deepEqual(r.conflicts, ["c.txt"]);
});

test("non-utf8 read throws a clear error", () => {
  freshSandbox();
  store.branchCreate(BR, null);
  store.writeFile(BR, "bin.dat", Buffer.from([0xff, 0xfe, 0x00, 0x01]), "binary");
  assert.throws(() => store.readText(BR, "bin.dat"), /not valid UTF-8/);
});

test("listFiles reports only branch deltas, not the disk tree", () => {
  const { disk } = freshSandbox();
  store.branchCreate(BR, null);
  fs.writeFileSync(path.join(disk, "ondisk.txt"), "x\n");
  store.writeFile(BR, "touched.txt", Buffer.from("y\n"), "t");
  assert.deepEqual(store.listFiles(BR, "", true), ["touched.txt"]);
});
