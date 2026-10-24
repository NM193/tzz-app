// Run: node --test src/lib/queue.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { fileItem, pendingFile } from "./queue.ts";

test("a file waits as a pill named after it, with nothing to read", () => {
  const pill = pendingFile("/Users/x/Documents/Snimci/Lecture 3.mp4");
  assert.equal(pill.url, "/Users/x/Documents/Snimci/Lecture 3.mp4");
  assert.equal(pill.title, "Lecture 3.mp4");
  assert.equal(pill.file, true);
  assert.equal(pill.failed, false);
});

test("a recording from Record always asks for a transcript", () => {
  assert.equal(fileItem("/x/a.mp3", "sr", true).wantTranscript, true);
  assert.equal(fileItem("/x/a.mp3").wantTranscript, undefined);
});
