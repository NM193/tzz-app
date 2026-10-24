// Run: node --test src/lib/language.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { choiceFor, choicesFor, detectedLabel, languageName } from "./language.ts";

test("Serbian, Croatian and Bosnian are all Serbian, which sends sr", () => {
  for (const code of ["sr", "hr", "bs"]) assert.equal(choiceFor(code), "sr");
  assert.equal(choiceFor("en"), "en");
  assert.equal(choiceFor("de"), "de");
});

test("Serbian and English are always offered; anything else gets its own button", () => {
  assert.deepEqual(choicesFor(null), ["sr", "en"]);
  assert.deepEqual(choicesFor("hr"), ["sr", "en"]);
  assert.deepEqual(choicesFor("en"), ["sr", "en"]);
  assert.deepEqual(choicesFor("de"), ["sr", "en", "de"]);
});

test("languages are named in English", () => {
  assert.equal(languageName("sr"), "Serbian");
  assert.equal(languageName("de"), "German");
});

test("the label says what Whisper heard and how sure it was", () => {
  assert.equal(detectedLabel({ code: "hr", probability: 0.526195 }), "Detected · Croatian 53%");
  assert.equal(detectedLabel({ code: "en", probability: 0.381886 }), "Detected · English 38%");
});
