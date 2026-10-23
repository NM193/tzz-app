# Release notes: written as we go, shown before an update

**Status:** agreed and built 2026-10-23. The first release to carry it is shown by the old notice.

## What is wanted

When the app changes, the people using it get an update that says what is new
and what was fixed. The update notice already exists and already shows a
`notes` string; what is missing is the habit of writing the notes while the
work happens, a shape for them, and anything for someone who skipped a version.

Decided with the owner:

1. **A release happens when the owner says so** ("objavi"), never on its own.
   Claude shows what is under Unreleased and proposes a version; only once the
   owner agrees to that version does it run `release.sh <version>`, which
   pushes and publishes.
2. **Notes are written while working**, into `CHANGELOG.md`, one line per
   change, in the same commit as the change. Not reconstructed from git at
   release time: commit messages here describe the code, not what a user sees.
3. **They are shown in the existing update notice**, before Update is clicked,
   split into New / Improved / Fixed. Someone who skipped versions sees every
   version since their own, up to the ten the manifest carries. The cap is
   deliberate: further back than that is history, not news. No card after the
   update, no page in Settings.

## CHANGELOG.md

At the root of the repo, newest first:

```markdown
# Changelog

What changed in each version of Tzz App, newest first. The app shows these
notes when it offers an update, so each line is written for the person using
it, not about the code.

## Unreleased

### New

### Improved

### Fixed

## 0.1.2 — 2026-10-02

### Fixed
- An update the app announces can now actually be downloaded.
```

Rules the parser enforces:

- A `## ` line starts a version: `## Unreleased`, or `## X.Y.Z` followed by
  anything (the date, which `stamp` writes as ` — YYYY-MM-DD`). Any other
  `## ` line, a misspelt Unreleased say, is an error. A version appears once,
  and Unreleased, when present, is the first.
- Inside a version only `### New`, `### Improved` and `### Fixed` are allowed,
  shown in that order. Anything else, `### Fixes` say, is an error. A heading
  that repeats inside one version adds to the same section.
- A note is a line starting with `- ` with text after it. A line indented by
  two spaces continues the note above it. Blank lines are ignored. Any other
  text inside a version is an error. Text before the first `## ` (the title
  and intro) is ignored, except a note or a `##` line there, which is a
  version heading gone wrong and an error.
- Every error names its line, so a typo stops a release instead of silently
  dropping a note.

Rules for whoever writes in it (they go into CLAUDE.md):

- Every change someone using the app would notice gets one line under
  Unreleased, in the same commit. English, plain, what they can now do or what
  stopped going wrong.
- Changes nobody using the app would notice (docs, refactors, scripts, tests)
  get no line.

0.1.1 and 0.1.2 are filled in from their GitHub release notes and commits, so
the history starts complete.

## scripts/changelog.py

The one place that reads the file. `release.sh` calls it; it keeps Python out
of bash. `--file PATH` points it at another copy, for trying it out.

- `stamp <version>` — turns Unreleased into `## <version> — <today>` and puts a
  fresh Unreleased, holding the three empty headings, above it. Empty headings
  are dropped from the stamped version. Checked in this order, each refusal a
  plain sentence:
  1. a version that is not `X.Y.Z` is refused;
  2. the newest version already in the file, with Unreleased empty, changes
     nothing and succeeds: that is a release run again after it fell over,
     which `release.sh` supports on purpose. With notes waiting under
     Unreleased it is refused: that is the current version typed for the
     next one, and it would re-release the current version;
  3. any other version that is not newer than the newest is refused,
     including one already in the file. `release.sh 0.1.1` typed for 0.1.3
     has to stop here, before it commits, pushes and re-tags an old version;
  4. an Unreleased with no notes is refused.
- `markdown <version>` — that version's notes as markdown, for the GitHub
  release page.
- `text <version>` — the same as plain text (`New`, then `- ` lines, a blank
  line between sections), for the manifest's `notes`.
- `changes <version>` — JSON: that version and up to nine before it, newest
  first, each `{"version", "new", "improved", "fixed"}` with all three lists
  always present.

## release.sh

- Called as `release.sh <version>`. A second argument is refused with a line
  saying the notes now come from CHANGELOG.md.
- `stamp` runs after the cheap checks (key, pubkey, repo) and before the
  version is written into `tauri.conf.json`, so a stamp never happens for a
  release that could not start, and the stamped CHANGELOG.md goes into the
  existing `Version X` commit.
- The GitHub release gets `markdown` as its notes.
- `latest.json` gets `text` as `notes` and `changes` as a new top-level field.
- The existing check, which downloads this release's own `latest.json`, also
  requires `changes[0].version` to be this version and to hold at least one
  note.

Why an extra field is safe: tauri-plugin-updater 2.13.1 (the locked version)
reads the manifest into a struct without `deny_unknown_fields`, so copies
already installed ignore `changes`, and it hands the whole response to the
frontend as `Update.rawJson`. Both checked in the plugin source.

## The update notice

- `src/lib/releaseNotes.ts` holds the logic: from `update.rawJson` and
  `update.currentVersion`, the versions newer than the running one, newest
  first. Entries that are malformed, or have no notes, are skipped. Versions
  compare as three numbers. It takes plain values (`unknown`, `string`) and
  imports nothing, so Node can run its test directly.
- `UpdateNotice` draws them under the existing `Version X` line. Section labels
  (NEW, IMPROVED, FIXED) and version numbers are things you only read, so they
  are mono, uppercase and dim: the existing `.eyebrow` look, in `--caption`,
  like every other such label. The notes themselves stay the size and colour
  of today's notes text. Every version other than the offered one starts with
  a hairline that separates it from the newer one, then a label with its
  number.
- When there is nothing usable in `changes`, the notice shows `update.body` as
  it does today, so a release made by hand still says something.
- Styles sit beside the existing `.update` rules and use only the colour
  variables at the top of `styles.css`.

## The next release, and the one after

The next update is shown by the notice of the version people have now, which
only knows `notes`. That is why `notes` is plain text that reads well on its
own. The new notice is first seen on the update after that.

## CLAUDE.md

- The writing rules above, and the release flow from decision 1, including
  that the version waits for the owner's yes.
- `scripts/release.sh <version> "notes"` becomes `scripts/release.sh <version>`.
- The two new test commands under Commands.

The project memory that repeats the old two-argument form is corrected too.

## Checking it

- `scripts/test_changelog.py`, plain `unittest` run with `python3`: a first
  stamp, the same stamp again, an older version, an empty Unreleased, a
  misspelt heading, and the three outputs.
- `src/lib/releaseNotes.test.ts`, `node:test` run with `node --test`. Node 22
  runs TypeScript directly, so nothing is installed. Covers skipped versions,
  a missing `changes`, malformed entries and numeric version order.
  `tsconfig.json` leaves `*.test.ts` out, since the app's types know nothing
  of Node.
- `npm run build:vite` for types.
- `release.sh` is not run, because running it publishes. `bash -n` for
  syntax. Its two new pieces of Python, the manifest and the check of
  `changes`, are pulled out of the script as written and run in the scratchpad
  against a sample CHANGELOG and a fake archive.
- The notice is looked at once with a sample update. A temporary change in
  `UpdateNotice` sets it directly, the page is opened from the Vite dev
  server, and the change is reverted before anything is committed. The
  updater's endpoint is never pointed anywhere else: `release.sh` commits with
  `git add -A`, and a forgotten endpoint would ship to every installed copy.

## Not doing

A card after updating, a changelog page in Settings, releasing automatically,
notes generated from commits, translated notes, and the Gatekeeper and minimum
macOS problems found on 2026-10-23 (a separate decision).
