// Run: node --test src/lib/naming.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Clash, Holds } from "./api";
import type { QueueItem } from "./queue";
import {
  asRun,
  fromFolder,
  refusalLine,
  refused,
  replaceLine,
  rowsFrom,
  stateLine,
  toCandidates,
  type NameRow,
} from "./naming.ts";

const file = (id: string, target: string): QueueItem => ({
  id,
  kind: "file",
  label: target.split("/").pop() ?? target,
  target,
  status: "waiting",
});

const link = (id: string, title: string, videoId?: string): QueueItem => ({
  id,
  kind: "url",
  label: title,
  target: `https://youtu.be/${id}`,
  status: "waiting",
  named: true,
  videoId,
});

const none: Holds = { audio: false, transcript: false, video: false, screens: false, notes: false, modified: 0 };

const clash = (key: string, patch: Partial<Clash> = {}): Clash => ({
  key,
  folder: key,
  inLibrary: false,
  sameAs: null,
  replaceable: false,
  suggestion: null,
  usable: true,
  holds: none,
  ...patch,
});

test("a link is checked by its title and id, a file by its path", () => {
  assert.deepEqual(toCandidates([link("a", "Talk", "abc"), file("b", "/x/Lecture 1.mp4")]), [
    { key: "a", title: "Talk", videoId: "abc" },
    { key: "b", path: "/x/Lecture 1.mp4", name: null },
  ]);
});

test("rows are the jobs the first check found taken, New name chosen", () => {
  const rows = rowsFrom([
    clash("a", { inLibrary: true, suggestion: "Talk (2)" }),
    clash("b"),
    clash("c", { sameAs: "a", suggestion: "Talk (3)" }),
  ]);
  assert.deepEqual(rows, [
    { key: "a", choice: "rename", name: "Talk (2)" },
    { key: "c", choice: "rename", name: "Talk (3)" },
  ]);
});

test("the batch as it would run: skipped out, replaced marked, renamed renamed", () => {
  const items = [
    link("a", "Talk", "abc"),
    file("b", "/x/Lecture 1.mp4"),
    file("c", "/y/Lecture 1.mp4"),
    file("e", "/z/Lecture 1.mp4"),
    link("d", "Other", "def"),
  ];
  const run = asRun(items, [
    { key: "a", choice: "rename", name: " Talk (2) " },
    { key: "b", choice: "replace", name: "Lecture 1 (2)" },
    { key: "c", choice: "skip", name: "Lecture 1 (3)" },
    { key: "e", choice: "rename", name: "Lecture 1 (4)" },
  ]);
  assert.deepEqual(run.map((item) => item.id), ["a", "b", "e", "d"]);
  assert.equal(run[0].label, "Talk (2)");
  assert.equal(run[0].named, true);
  assert.equal(run[1].replace, true);
  assert.equal(run[1].name, undefined);
  assert.equal(run[2].name, "Lecture 1 (4)");
  assert.equal(run[2].label, "Lecture 1.mp4");
  assert.equal(run[3], items[4]);
});

test("a New name the library has, or that cannot be a folder, is refused", () => {
  const rows: NameRow[] = [
    { key: "a", choice: "rename", name: "Talk" },
    { key: "b", choice: "rename", name: ".x" },
  ];
  const found = refused(rows, [clash("a", { inLibrary: true }), clash("b", { usable: false })], null);
  assert.deepEqual([...found].sort(), ["a", "b"]);
});

test("Replace is never refused for the folder it replaces", () => {
  const rows: NameRow[] = [{ key: "a", choice: "replace", name: "Talk (2)" }];
  assert.equal(refused(rows, [clash("a", { inLibrary: true })], null).size, 0);
});

test("a clash with a job that has no row is pinned on the row, above or below", () => {
  const rows: NameRow[] = [{ key: "row", choice: "rename", name: "Lecture 1 (2)" }];
  assert.deepEqual([...refused(rows, [clash("other"), clash("row", { sameAs: "other" })], "row")], ["row"]);
  assert.deepEqual([...refused(rows, [clash("row"), clash("other", { sameAs: "row" })], "row")], ["row"]);
});

test("between two rows, the one just edited is refused, or else the later one", () => {
  const rows: NameRow[] = [
    { key: "a", choice: "rename", name: "X" },
    { key: "b", choice: "rename", name: "X" },
  ];
  const found = [clash("a"), clash("b", { sameAs: "a" })];
  assert.deepEqual([...refused(rows, found, "a")], ["a"]);
  assert.deepEqual([...refused(rows, found, null)], ["b"]);
});

test("what Replace takes away, from what is there", () => {
  assert.equal(
    replaceLine({ ...none, audio: true, transcript: true, screens: true, notes: true }),
    "MP3, transcript and screen text go · Notes stay",
  );
  assert.equal(replaceLine({ ...none, audio: true }), "MP3 goes");
  assert.equal(replaceLine({ ...none, audio: true, video: true }), "MP3 and video go");
});

test("a link says when it was finished, a file where it comes from", () => {
  const holds = { ...none, audio: true, transcript: true, notes: true, modified: Date.UTC(2026, 9, 24, 12) / 1000 };
  assert.equal(
    stateLine(link("a", "Talk", "abc"), clash("a", { inLibrary: true, holds })),
    "Finished 24 Oct · MP3 · Transcript · Notes",
  );
  assert.equal(
    stateLine(file("b", "/Users/someone/Downloads/Week 1/Lecture 1.mp4"), clash("b", { inLibrary: true, holds: { ...none, audio: true } })),
    "From ~/Downloads/Week 1 · MP3",
  );
  assert.equal(
    stateLine(file("c", "/Users/someone/Downloads/Week 2/Lecture 1.mp4"), clash("c", { sameAs: "b" })),
    "From ~/Downloads/Week 2 · Same name as another in this start",
  );
  assert.equal(fromFolder("/Volumes/Disk/Lecture.mp4"), "/Volumes/Disk");
});

test("a refused row says why", () => {
  assert.equal(refusalLine({ key: "a", choice: "rename", name: "  " }, clash("a", { usable: false })), "Needs a name");
  assert.equal(refusalLine({ key: "a", choice: "rename", name: ".x" }, clash("a", { usable: false })), "That name cannot be a folder");
  assert.equal(refusalLine({ key: "a", choice: "rename", name: "Talk" }, clash("a", { inLibrary: true })), "That name is taken too");
});
