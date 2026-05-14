/**
 * Store unit tests. Mirrors git-fs/src/store.rs #[cfg(test)] block.
 * Run with: npm test
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { Buffer } from "node:buffer";
import { Store } from "../src/store.js";

async function tempStore(): Promise<{ dir: string; store: Store }> {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "gitfs-test-"));
  const store = await Store.init(path.join(dir, "repo"));
  return { dir, store };
}

test("write_and_read_file", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "hello.txt", Buffer.from("world"), "add hello");
  const content = await store.readFile("main", "hello.txt");
  assert.equal(Buffer.from(content).toString("utf-8"), "world");
});

test("write_nested_path", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "src/lib.rs", Buffer.from("pub fn f() {}"), "add lib");
  const content = await store.readFile("main", "src/lib.rs");
  assert.equal(Buffer.from(content).toString("utf-8"), "pub fn f() {}");
});

test("branch_create_list_delete", async () => {
  const { store } = await tempStore();
  await store.branchCreate("alpha");
  await store.branchCreate("beta");
  const branches = await store.branchList();
  assert.ok(branches.includes("alpha"));
  assert.ok(branches.includes("beta"));
  await store.branchDelete("alpha");
  const after = await store.branchList();
  assert.ok(!after.includes("alpha"));
});

test("list_files_flat_and_recursive", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "a.txt", Buffer.from("a"), "a");
  await store.writeFile("main", "sub/b.txt", Buffer.from("b"), "b");

  const flat = await store.listFiles("main", "", false);
  const flatNames = flat.map((e) => e.path);
  assert.ok(flatNames.includes("a.txt"));
  assert.ok(flatNames.includes("sub"));

  const rec = await store.listFiles("main", "", true);
  const recNames = rec.map((e) => e.path);
  assert.ok(recNames.includes("a.txt"));
  assert.ok(recNames.includes("sub/b.txt"));
});

test("remove_file", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "bye.txt", Buffer.from("x"), "add");
  await store.removeFile("main", "bye.txt", "rm bye");
  await assert.rejects(() => store.readFile("main", "bye.txt"));
});

test("log_entries", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "f1.txt", Buffer.from("1"), "first commit");
  await store.writeFile("main", "f2.txt", Buffer.from("2"), "second commit");
  const entries = await store.log("main", 10);
  const msgs = entries.map((e) => e.message);
  assert.ok(msgs.some((m) => m.startsWith("first commit")));
  assert.ok(msgs.some((m) => m.startsWith("second commit")));
  const firstOid = entries[0]?.oid ?? "";
  assert.equal(firstOid.length, 40);
});

test("diff_shows_changes", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "x.txt", Buffer.from("hello\n"), "v1");
  const v1 = (await store.log("main", 1))[0]!.oid;
  await store.writeFile("main", "x.txt", Buffer.from("hello\nworld\n"), "v2");
  const diff = await store.diff(v1, "main");
  assert.ok(diff.includes("+world"));
});

test("replace_file_unique_match", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "f.txt", Buffer.from("hello\nworld\n"), "v1");
  await store.replaceFile("main", "f.txt", "world", "universe", "swap");
  const out = Buffer.from(await store.readFile("main", "f.txt")).toString("utf-8");
  assert.equal(out, "hello\nuniverse\n");
});

test("replace_file_rejects_non_unique", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "f.txt", Buffer.from("a\na\n"), "v1");
  await assert.rejects(() => store.replaceFile("main", "f.txt", "a", "b", "v2"), /matches 2/);
});

test("patch_file_line_range", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "f.txt", Buffer.from("one\ntwo\nthree\n"), "v1");
  await store.patchFile("main", "f.txt", 2, 2, "TWO", "edit l2");
  const out = Buffer.from(await store.readFile("main", "f.txt")).toString("utf-8");
  assert.equal(out, "one\nTWO\nthree\n");
});

test("merge_auto_resolves_non_overlapping_edits", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  const base = "one\ntwo\nthree\nfour\nfive\n";
  await store.writeFile("main", "f.txt", Buffer.from(base), "v0");
  await store.branchCreate("a", "main");
  await store.branchCreate("b", "main");
  await store.writeFile("a", "f.txt", Buffer.from("ONE\ntwo\nthree\nfour\nfive\n"), "edit l1");
  await store.writeFile("b", "f.txt", Buffer.from("one\ntwo\nthree\nfour\nFIVE\n"), "edit l5");
  await store.branchCreate("merged", "a");
  const result = await store.merge("main", "a", "b", "merged", "auto");
  assert.equal(result.kind, "clean");
  if (result.kind === "clean") {
    const blob = await store.readFile("merged", "f.txt");
    assert.equal(Buffer.from(blob).toString("utf-8"), "ONE\ntwo\nthree\nfour\nFIVE\n");
  }
});

test("merge_reports_conflict_on_overlapping_edits", async () => {
  const { store } = await tempStore();
  await store.branchCreate("main");
  await store.writeFile("main", "f.txt", Buffer.from("one\ntwo\n"), "v0");
  await store.branchCreate("a", "main");
  await store.branchCreate("b", "main");
  await store.writeFile("a", "f.txt", Buffer.from("ONE\ntwo\n"), "a edit");
  await store.writeFile("b", "f.txt", Buffer.from("uno\ntwo\n"), "b edit");
  const result = await store.merge("main", "a", "b", null, "conflict");
  assert.equal(result.kind, "conflicts");
  if (result.kind === "conflicts") {
    assert.equal(result.conflicts[0]?.path, "f.txt");
  }
});

test("session_trailer_appended_for_agent_branch", () => {
  const out = Store.__withSessionTrailerForTest("write hello", "agent/abc-123");
  assert.match(out, /Session-Id: abc-123/);
});

test("session_trailer_idempotent", () => {
  const msg = "first\n\nSession-Id: abc-123\n";
  const out = Store.__withSessionTrailerForTest(msg, "agent/abc-123");
  assert.equal(out, msg);
});

test("session_trailer_skipped_for_non_agent_branch", () => {
  const out = Store.__withSessionTrailerForTest("write hello", "main");
  assert.equal(out, "write hello");
});
