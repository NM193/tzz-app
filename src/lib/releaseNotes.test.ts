// Run: node --test src/lib/releaseNotes.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { compareVersions, notesSince } from "./releaseNotes.ts";

const fixedIn = (version: string) => ({
  version,
  new: [],
  improved: [],
  fixed: [`Fixed in ${version}.`],
});

test("every version newer than the running one, newest first", () => {
  const manifest = { changes: [fixedIn("0.1.2"), fixedIn("0.1.4"), fixedIn("0.1.3"), fixedIn("0.1.1")] };
  assert.deepEqual(
    notesSince(manifest, "0.1.2").map((entry) => entry.version),
    ["0.1.4", "0.1.3"],
  );
});

test("nothing, when the manifest has no changes list", () => {
  assert.deepEqual(notesSince({ version: "0.1.3", notes: "Plain text." }, "0.1.2"), []);
  assert.deepEqual(notesSince({ changes: "not a list" }, "0.1.2"), []);
  assert.deepEqual(notesSince(null, "0.1.2"), []);
});

test("entries that are malformed or say nothing are skipped", () => {
  const manifest = {
    changes: [
      { version: "0.1.5", new: ["Kept."], improved: "not a list", fixed: [3, "", "Also kept."] },
      { version: "0.1.4", new: [], improved: [], fixed: [] },
      { version: "0.1", fixed: ["No patch number."] },
      "not an entry",
    ],
  };
  assert.deepEqual(notesSince(manifest, "0.1.2"), [
    { version: "0.1.5", new: ["Kept."], improved: [], fixed: ["Also kept."] },
  ]);
});

test("versions compare as numbers, not as text", () => {
  assert.ok(compareVersions("0.10.0", "0.9.0") > 0);
  assert.ok(compareVersions("0.1.2", "0.1.10") < 0);
  assert.ok(compareVersions("1.0.0", "0.99.99") > 0);
  assert.equal(compareVersions("0.1.2", "0.1.2"), 0);
});
