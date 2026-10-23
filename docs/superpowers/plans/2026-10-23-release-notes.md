# Release Notes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Notes about what changed are written into `CHANGELOG.md` as the work happens, `release.sh` turns them into the update's notes, and the update notice shows New / Improved / Fixed for every version since the one installed.

**Architecture:** `CHANGELOG.md` is the single source. `scripts/changelog.py` is its only reader: it stamps Unreleased with the version and renders the notes as markdown (GitHub release), plain text (`notes` in `latest.json`, all that old copies can show) and JSON (`changes` in `latest.json`, the last ten versions). The app reads `changes` from `Update.rawJson` through a small pure module, `src/lib/releaseNotes.ts`, and `UpdateNotice` draws it, falling back to `notes`.

**Tech Stack:** Python 3 standard library (`unittest`), bash, React 18 + TypeScript, `node:test` on Node 22 (runs `.ts` directly), tauri-plugin-updater 2.13.1.

**Spec:** `docs/superpowers/specs/2026-10-23-release-notes-design.md`

**Ground rules for this repo:**
- Do not run `scripts/release.sh`. Running it builds, pushes and publishes a release.
- Do not touch `plugins.updater.endpoints` in `src-tauri/tauri.conf.json`, not even temporarily.
- No Rust changes are needed.
- Commit only once the owner has said commits are fine. Commit messages in this repo are plain sentences, not `feat:` prefixes, and end with the `Co-Authored-By` line shown in each commit step.

---

## File structure

| File | Status | Responsibility |
|---|---|---|
| `scripts/changelog.py` | create | The only reader of `CHANGELOG.md`: parse strictly, stamp, render markdown / text / JSON |
| `scripts/test_changelog.py` | create | `unittest` tests for the above |
| `.gitignore` | modify | Ignore `__pycache__`, which the tests leave behind and `release.sh`'s `git add -A` would commit |
| `CHANGELOG.md` | create | The notes, newest first, with Unreleased on top |
| `scripts/release.sh` | modify | Takes only `<version>`; stamps; uses the three renderings; checks `changes` |
| `src/lib/releaseNotes.ts` | create | Pure: `notesSince(rawJson, currentVersion)`, `compareVersions`, `SECTIONS` |
| `src/lib/releaseNotes.test.ts` | create | `node:test` tests for the above |
| `tsconfig.json` | modify | Leave `*.test.ts` out of the app's typecheck (no Node types there) |
| `src/components/UpdateNotice.tsx` | modify | Draw versions and sections; fall back to `update.body` |
| `src/styles.css` | modify | A few rules beside the existing `.update` block |
| `CLAUDE.md` | modify | One-argument release, the writing rules, the release flow, test commands |

---

### Task 1: `scripts/changelog.py`, test first

**Files:**
- Modify: `.gitignore`
- Create: `scripts/test_changelog.py`
- Create: `scripts/changelog.py`

- [ ] **Step 0: Keep Python's bytecode out of git**

Importing `changelog` from the tests writes `scripts/__pycache__/`, and `release.sh` commits with `git add -A`. Append to `.gitignore`:

```gitignore

# Written by Python when scripts/test_changelog.py imports changelog.py.
__pycache__
```

- [ ] **Step 1: Write the failing tests**

Create `scripts/test_changelog.py`:

```python
"""Tests for changelog.py. Run: python3 scripts/test_changelog.py"""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import changelog

SAMPLE = """# Changelog

Intro.

## Unreleased

### New
- Search inside a transcript.

### Improved

### Fixed
- A long note that goes on
  over two lines.

## 0.1.2 — 2026-10-02

### Fixed
- An update can be downloaded.

## 0.1.1 — 2026-10-02

### New
- The app updates itself.
"""


class Stamp(unittest.TestCase):
    def test_unreleased_becomes_the_version_under_a_fresh_unreleased(self):
        stamped = changelog.stamp(SAMPLE, "0.2.0", "2026-10-24")
        self.assertIn(
            "Intro.\n\n## Unreleased\n\n### New\n\n### Improved\n\n### Fixed\n\n"
            "## 0.2.0 — 2026-10-24\n\n### New\n- Search inside a transcript.\n\n"
            "### Fixed\n- A long note that goes on over two lines.\n\n"
            "## 0.1.2 — 2026-10-02\n",
            stamped,
        )
        self.assertTrue(stamped.endswith("- The app updates itself.\n"))

    def test_the_newest_version_again_changes_nothing(self):
        stamped = changelog.stamp(SAMPLE, "0.2.0", "2026-10-24")
        self.assertEqual(changelog.stamp(stamped, "0.2.0", "2026-10-25"), stamped)

    def test_the_newest_version_again_is_refused_while_unreleased_has_notes(self):
        # 0.1.2 typed for 0.1.3 would otherwise re-release 0.1.2.
        with self.assertRaisesRegex(changelog.ChangelogError, "0.1.2 is already in CHANGELOG.md"):
            changelog.stamp(SAMPLE, "0.1.2", "2026-10-24")

    def test_an_older_version_is_refused_even_when_it_is_in_the_file(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "0.1.1 is not newer than 0.1.2"):
            changelog.stamp(SAMPLE, "0.1.1", "2026-10-24")

    def test_an_empty_unreleased_is_refused(self):
        stamped = changelog.stamp(SAMPLE, "0.2.0", "2026-10-24")
        with self.assertRaisesRegex(changelog.ChangelogError, "Nothing is written under Unreleased"):
            changelog.stamp(stamped, "0.2.1", "2026-10-25")

    def test_a_version_is_three_numbers(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "three numbers"):
            changelog.stamp(SAMPLE, "0.2", "2026-10-24")


class Parse(unittest.TestCase):
    def test_a_misspelt_section_names_its_line(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "line 7: '### Fixes'"):
            changelog.parse(SAMPLE.replace("### New", "### Fixes", 1))

    def test_a_misspelt_unreleased_is_not_a_version(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "line 5: '## Unrelased' is not a version"):
            changelog.parse(SAMPLE.replace("## Unreleased", "## Unrelased"))

    def test_a_note_above_every_version_is_refused(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "line 3: '- Stray note.' is above every version"):
            changelog.parse(SAMPLE.replace("Intro.", "- Stray note."))

    def test_text_that_is_not_a_note_is_refused(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "line 8: .* is not a note"):
            changelog.parse(SAMPLE.replace("- Search", "Search"))

    def test_a_version_appears_once(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "0.1.1 appears twice"):
            changelog.parse(SAMPLE + "\n## 0.1.1 — 2026-10-01\n")

    def test_unreleased_comes_first(self):
        moved = SAMPLE.replace("## Unreleased", "## 0.1.3 — 2026-10-03", 1) + "\n## Unreleased\n"
        with self.assertRaisesRegex(changelog.ChangelogError, "Unreleased has to be the first version"):
            changelog.parse(moved)

    def test_a_repeated_section_adds_to_the_same_one(self):
        doubled = SAMPLE.replace("### Improved\n", "### Fixed\n- Also fixed.\n", 1)
        unreleased = changelog.parse(doubled)[0]
        self.assertEqual(
            unreleased.notes["Fixed"],
            ["Also fixed.", "A long note that goes on over two lines."],
        )


class Outputs(unittest.TestCase):
    def setUp(self):
        self.stamped = changelog.stamp(SAMPLE, "0.2.0", "2026-10-24")

    def test_markdown_has_only_the_sections_with_notes(self):
        self.assertEqual(
            changelog.markdown(self.stamped, "0.2.0"),
            "### New\n- Search inside a transcript.\n\n"
            "### Fixed\n- A long note that goes on over two lines.\n",
        )

    def test_text_reads_on_its_own(self):
        self.assertEqual(
            changelog.plain(self.stamped, "0.2.0"),
            "New\n- Search inside a transcript.\n\n"
            "Fixed\n- A long note that goes on over two lines.\n",
        )

    def test_changes_start_at_the_version_newest_first(self):
        self.assertEqual(
            changelog.changes(self.stamped, "0.1.2"),
            [
                {"version": "0.1.2", "new": [], "improved": [], "fixed": ["An update can be downloaded."]},
                {"version": "0.1.1", "new": ["The app updates itself."], "improved": [], "fixed": []},
            ],
        )

    def test_changes_keep_ten_versions(self):
        many = "".join(
            f"## 0.0.{n} — 2026-01-01\n\n### Fixed\n- Fix {n}.\n\n" for n in range(12, 0, -1)
        )
        self.assertEqual(
            [entry["version"] for entry in changelog.changes(many, "0.0.12")],
            [f"0.0.{n}" for n in range(12, 2, -1)],
        )

    def test_a_version_that_is_not_there_is_an_error(self):
        with self.assertRaisesRegex(changelog.ChangelogError, "0.9.9 is not in CHANGELOG.md"):
            changelog.markdown(SAMPLE, "0.9.9")


class CommandLine(unittest.TestCase):
    def test_stamp_writes_the_file_and_a_refusal_exits_with_one(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "CHANGELOG.md"
            path.write_text(SAMPLE, encoding="utf-8")
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                self.assertEqual(changelog.main(["stamp", "0.2.0", "--file", str(path)]), 0)
                self.assertIn("## 0.2.0 — ", path.read_text(encoding="utf-8"))
                self.assertEqual(changelog.main(["stamp", "0.2.1", "--file", str(path)]), 1)
            self.assertIn("Nothing is written under Unreleased", said.getvalue())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `python3 scripts/test_changelog.py`
Expected: `ModuleNotFoundError: No module named 'changelog'`

- [ ] **Step 3: Write `scripts/changelog.py`**

```python
#!/usr/bin/env python3
"""Read CHANGELOG.md, where the release notes are written as the work happens.

The app shows these notes when it offers an update, so the file is read
strictly: a misspelt heading stops a release instead of quietly dropping the
notes under it.

    python3 scripts/changelog.py stamp 0.2.0      Unreleased becomes 0.2.0
    python3 scripts/changelog.py markdown 0.2.0   the notes, for the GitHub release
    python3 scripts/changelog.py text 0.2.0       the same as plain text
    python3 scripts/changelog.py changes 0.2.0    0.2.0 and the nine before it, as JSON

--file PATH reads another copy instead, for trying things out.
"""

import argparse
import datetime
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

CHANGELOG = Path(__file__).resolve().parent.parent / "CHANGELOG.md"

UNRELEASED = "Unreleased"

# The only headings allowed inside a version, in the order they are shown.
SECTIONS = ("New", "Improved", "Fixed")

# How many versions the manifest carries: enough for anyone who skipped a few.
# Further back than that is history, not news.
KEEP = 10

NUMBERS = re.compile(r"\d+\.\d+\.\d+")


class ChangelogError(Exception):
    """What is wrong, in a sentence for the person releasing."""


@dataclass
class Version:
    name: str
    start: int  # the index of its `## ` line
    end: int  # the index just past its last line
    notes: dict = field(default_factory=lambda: {section: [] for section in SECTIONS})

    def is_empty(self):
        return not any(self.notes.values())


def order(name):
    return tuple(int(part) for part in name.split("."))


def parse(text):
    """Every version in the file, in the order written."""
    lines = text.splitlines()
    versions = []
    version = section = None

    for index, line in enumerate(lines):
        where = f"CHANGELOG.md line {index + 1}"

        if line.startswith("## "):
            words = line[3:].split()
            name = words[0] if words else ""
            if name != UNRELEASED and not NUMBERS.fullmatch(name):
                raise ChangelogError(
                    f"{where}: '{line}' is not a version. "
                    f"Write '## {UNRELEASED}' or '## 1.2.3 — 2026-01-31'."
                )
            if any(seen.name == name for seen in versions):
                raise ChangelogError(f"{where}: {name} appears twice.")
            if name == UNRELEASED and versions:
                raise ChangelogError(f"{where}: {UNRELEASED} has to be the first version.")
            if version:
                version.end = index
            version = Version(name, index, len(lines))
            versions.append(version)
            section = None

        elif version is None:
            # The title and the introduction. A note or a heading up here is a
            # version heading gone wrong, and the notes under it would be lost.
            if line.startswith(("- ", "##")):
                raise ChangelogError(
                    f"{where}: '{line}' is above every version. "
                    f"Notes go under '## {UNRELEASED}'."
                )

        elif line.startswith("### "):
            heading = line[4:].strip()
            if heading not in SECTIONS:
                raise ChangelogError(
                    f"{where}: '{line}' is not a section. Use ### New, ### Improved or ### Fixed."
                )
            section = heading  # a repeated heading adds to the same section

        elif line.startswith("- "):
            if section is None:
                raise ChangelogError(f"{where}: this note is not under New, Improved or Fixed.")
            note = line[2:].strip()
            if not note:
                raise ChangelogError(f"{where}: this note is empty.")
            version.notes[section].append(note)

        elif line.startswith("  ") and line.strip():
            if section is None or not version.notes[section]:
                raise ChangelogError(f"{where}: an indented line with no note above it.")
            version.notes[section][-1] += " " + line.strip()

        elif line.strip():
            raise ChangelogError(f"{where}: '{line}' is not a note. Notes start with '- '.")

    return versions


def render(notes):
    """Markdown lines for the sections that have notes, each followed by a blank line."""
    lines = []
    for section in SECTIONS:
        if notes[section]:
            lines += [f"### {section}"] + [f"- {note}" for note in notes[section]] + [""]
    return lines


def stamp(text, version, today):
    """The file with Unreleased turned into `version`, under a fresh Unreleased."""
    if not NUMBERS.fullmatch(version):
        raise ChangelogError(f"'{version}' is not a version. Use three numbers, like 0.2.0.")

    versions = parse(text)
    released = [each for each in versions if each.name != UNRELEASED]
    newest = max(released, key=lambda each: order(each.name), default=None)

    unreleased = versions[0] if versions and versions[0].name == UNRELEASED else None
    waiting = unreleased is not None and not unreleased.is_empty()

    # A release that fell over is run again, and finds its version already
    # stamped and Unreleased empty. Anything else is a version typed by mistake
    # -- the current one for the next, or an older one -- and has to stop here,
    # before release.sh commits, pushes and re-tags it.
    if newest and newest.name == version:
        if waiting:
            raise ChangelogError(
                f"{version} is already in CHANGELOG.md, and Unreleased has notes that "
                "are not in it. Release them as a newer version."
            )
        return text
    if newest and order(version) <= order(newest.name):
        raise ChangelogError(
            f"{version} is not newer than {newest.name}, the newest version in CHANGELOG.md."
        )

    if not waiting:
        raise ChangelogError(
            f"Nothing is written under {UNRELEASED} in CHANGELOG.md. Say what changed there first."
        )

    fresh = [f"## {UNRELEASED}", ""]
    for section in SECTIONS:
        fresh += [f"### {section}", ""]
    stamped = [f"## {version} — {today}", ""] + render(unreleased.notes)

    lines = text.splitlines()
    lines[unreleased.start:unreleased.end] = fresh + stamped
    return "\n".join(lines).rstrip("\n") + "\n"


def find(text, version):
    for each in parse(text):
        if each.name == version:
            if each.is_empty():
                raise ChangelogError(f"{version} has no notes in CHANGELOG.md.")
            return each
    raise ChangelogError(f"{version} is not in CHANGELOG.md.")


def markdown(text, version):
    """The notes for the GitHub release page."""
    return "\n".join(render(find(text, version).notes)).rstrip("\n") + "\n"


def plain(text, version):
    """The notes as plain text, for copies of the app that show nothing else."""
    notes = find(text, version).notes
    blocks = [
        "\n".join([section] + [f"- {note}" for note in notes[section]])
        for section in SECTIONS
        if notes[section]
    ]
    return "\n\n".join(blocks) + "\n"


def changes(text, version):
    """`version` and up to nine versions before it, newest first, for the manifest."""
    released = sorted(
        (each for each in parse(text) if each.name != UNRELEASED),
        key=lambda each: order(each.name),
        reverse=True,
    )
    names = [each.name for each in released]
    if version not in names:
        raise ChangelogError(f"{version} is not in CHANGELOG.md.")
    start = names.index(version)
    return [
        {
            "version": each.name,
            "new": each.notes["New"],
            "improved": each.notes["Improved"],
            "fixed": each.notes["Fixed"],
        }
        for each in released[start : start + KEEP]
    ]


def main(argv=None):
    parser = argparse.ArgumentParser(description="Read the release notes in CHANGELOG.md.")
    parser.add_argument("command", choices=("stamp", "markdown", "text", "changes"))
    parser.add_argument("version")
    parser.add_argument("--file", type=Path, default=CHANGELOG)
    args = parser.parse_args(argv)

    try:
        text = args.file.read_text(encoding="utf-8")
        if args.command == "stamp":
            stamped = stamp(text, args.version, datetime.date.today().isoformat())
            if stamped != text:
                args.file.write_text(stamped, encoding="utf-8")
        elif args.command == "markdown":
            sys.stdout.write(markdown(text, args.version))
        elif args.command == "text":
            sys.stdout.write(plain(text, args.version))
        else:
            print(json.dumps(changes(text, args.version), ensure_ascii=False))
    except FileNotFoundError:
        print(f"There is no {args.file}.", file=sys.stderr)
        return 1
    except ChangelogError as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `python3 scripts/test_changelog.py && git status --short`
Expected: `Ran 19 tests in ...` then `OK`; `git status` lists `.gitignore`, `scripts/changelog.py` and `scripts/test_changelog.py`, and no `__pycache__`.

- [ ] **Step 5: Commit**

```bash
git add .gitignore scripts/changelog.py scripts/test_changelog.py
git commit -m "$(cat <<'EOF'
A strict reader for the release notes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `CHANGELOG.md`, with the two releases already out

**Files:**
- Create: `CHANGELOG.md`

- [ ] **Step 1: Write the file**

0.1.1 was the first published version and 0.1.2 fixed its manifest. The notes come from the GitHub release notes and commits `c245f86`, `1b68377` and `3d9f854`. Unreleased starts empty: the line for this work's own visible change is added in Task 5, in the same commit as the change.

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

## 0.1.1 — 2026-10-01

### New
- The app checks for a new version when it opens, says what changed, and
  installs it only when you click Update.
- yt-dlp, ffmpeg and whisper-cli come inside the app, and Settings fetches a
  Whisper model, so there is nothing to install first.
- Record says when the Mac's own sound cannot be recorded yet, and links to
  BlackHole, which makes it possible.

### Improved
- The Chrome extension collects a whole course, every lesson under its real
  name, without clicking through the lessons one by one.
```

- [ ] **Step 2: Check the real file reads cleanly**

Run: `python3 scripts/changelog.py changes 0.1.2 | python3 -m json.tool`
Expected:

```json
[
    {
        "version": "0.1.2",
        "new": [],
        "improved": [],
        "fixed": [
            "An update the app announces can now actually be downloaded."
        ]
    },
    {
        "version": "0.1.1",
        "new": [
            "The app checks for a new version when it opens, says what changed, and installs it only when you click Update.",
            "yt-dlp, ffmpeg and whisper-cli come inside the app, and Settings fetches a Whisper model, so there is nothing to install first.",
            "Record says when the Mac's own sound cannot be recorded yet, and links to BlackHole, which makes it possible."
        ],
        "improved": [
            "The Chrome extension collects a whole course, every lesson under its real name, without clicking through the lessons one by one."
        ],
        "fixed": []
    }
]
```

Do not run `stamp` on the real file here; it is stamped only by a release.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "$(cat <<'EOF'
Write down what each version changed

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `release.sh` takes its notes from the changelog

**Files:**
- Modify: `scripts/release.sh` (header line 5, arguments at lines 27-33, before line 93, before line 123, the manifest at lines 151-166, the check after line 194)

- [ ] **Step 1: The usage in the header**

Replace:

```bash
#   bash scripts/release.sh 0.2.0 "What changed, in a sentence or two."
```

with:

```bash
#   bash scripts/release.sh 0.2.0
#
# What changed comes from CHANGELOG.md: whatever is under Unreleased becomes
# this version's notes, on the GitHub release and in the app's update notice.
```

- [ ] **Step 2: The arguments**

Replace:

```bash
version="${1:-}"
notes="${2:-A new version.}"

if [ -z "$version" ]; then
  echo "usage: bash scripts/release.sh <version> [notes]"
  exit 1
fi
```

with:

```bash
version="${1:-}"

if [ -z "$version" ]; then
  echo "usage: bash scripts/release.sh <version>"
  exit 1
fi

if [ "$#" -gt 1 ]; then
  echo "The notes come from CHANGELOG.md now: write them under Unreleased there."
  exit 1
fi
```

- [ ] **Step 3: Stamp before the version is written**

Immediately before the line `# The version the app reports, and the one an update is compared against.`, insert:

```bash
# What changed. Unreleased in CHANGELOG.md becomes this version here: after the
# checks above, so a release that cannot start never stamps one, and before the
# version is written, so the stamp goes into the same commit. A release run
# again finds its version already stamped and carries on; an older version is
# refused before anything is committed or pushed.
echo "== notes"
python3 scripts/changelog.py stamp "$version"
python3 scripts/changelog.py text "$version" | sed 's/^/   /'

```

- [ ] **Step 4: The GitHub release gets the markdown**

Immediately before the comment line `# Re-runnable on purpose. A release that got as far as uploading and then fell`, insert:

```bash
notes="$(python3 scripts/changelog.py markdown "$version")"

```

The `gh release edit` and `gh release create` lines already pass `--notes "$notes"` and stay as they are.

- [ ] **Step 5: The manifest gets `notes` as text and `changes`**

Replace:

```bash
python3 - "$version" "$notes" "$url" "$archive" <<'MANIFEST'
import datetime, json, sys
version, notes, url, archive = sys.argv[1:5]
json.dump({
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "darwin-aarch64": {
            "signature": open(archive + ".sig").read().strip(),
            "url": url,
        }
    },
}, open("latest.json", "w"), indent=2)
MANIFEST
```

with:

```bash
# The same notes twice. `notes` is plain text, because that is all a copy made
# before the notice read `changes` can show. `changes` is this version and the
# nine before it, so someone who skipped a few sees what they missed. The
# updater ignores fields it does not know and hands the whole file to the app.
# Assigned first, not substituted inline: `set -e` notices a failed assignment
# and would not notice a failed argument.
plain="$(python3 scripts/changelog.py text "$version")"
changes="$(python3 scripts/changelog.py changes "$version")"

python3 - "$version" "$plain" "$changes" "$url" "$archive" <<'MANIFEST'
import datetime, json, sys
version, notes, changes, url, archive = sys.argv[1:6]
json.dump({
    "version": version,
    "notes": notes,
    "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "platforms": {
        "darwin-aarch64": {
            "signature": open(archive + ".sig").read().strip(),
            "url": url,
        }
    },
    "changes": json.loads(changes),
}, open("latest.json", "w"), indent=2)
MANIFEST
```

- [ ] **Step 6: The check also reads `changes`**

Replace:

```bash
echo "   signature:  matches this build"
rm -rf "$here"
```

with:

```bash
echo "   signature:  matches this build"

# And the notes the notice reads: `changes` has to open with this version, and
# say something, or the update would be offered with nothing to tell.
if ! python3 - "$here/latest.json" "$version" <<'NOTES'
import json, sys
manifest, version = sys.argv[1:3]
first = (json.load(open(manifest)).get("changes") or [{}])[0]
sys.exit(0 if first.get("version") == version
         and any(first.get(key) for key in ("new", "improved", "fixed")) else 1)
NOTES
then
  echo "   the manifest's changes do not open with $version's notes."
  exit 1
fi
echo "   notes:      $version's open the changes"
rm -rf "$here"
```

- [ ] **Step 7: Syntax**

Run: `bash -n scripts/release.sh && echo syntax ok`
Expected: `syntax ok`

- [ ] **Step 8: Run the two new pieces of Python as written, against a fake release**

The manifest and the check run only after a real release has pushed and published, so they are pulled out of the script and run here. Run from the repo root:

```bash
S="/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/release-check"
rm -rf "$S" && mkdir -p "$S"
cp CHANGELOG.md "$S/CHANGELOG.md"
python3 - "$S/CHANGELOG.md" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read().replace("### Fixed\n", "### Fixed\n- Something a user would notice.\n", 1)
open(path, "w").write(text)
PY
python3 scripts/changelog.py --file "$S/CHANGELOG.md" stamp 0.1.3
printf 'not really an archive' > "$S/app.tar.gz"
printf 'SIGNATURE' > "$S/app.tar.gz.sig"
sed -n "/<<'MANIFEST'$/,/^MANIFEST$/p" scripts/release.sh | sed '1d;$d' > "$S/manifest.py"
sed -n "/<<'NOTES'$/,/^NOTES$/p" scripts/release.sh | sed '1d;$d' > "$S/check.py"
plain="$(python3 scripts/changelog.py --file "$S/CHANGELOG.md" text 0.1.3)"
changes="$(python3 scripts/changelog.py --file "$S/CHANGELOG.md" changes 0.1.3)"
(cd "$S" && python3 manifest.py 0.1.3 "$plain" "$changes" "https://example.invalid/Tzz.App.app.tar.gz" "$S/app.tar.gz")
cat "$S/latest.json"
python3 "$S/check.py" "$S/latest.json" 0.1.3 && echo "check: accepts 0.1.3"
python3 "$S/check.py" "$S/latest.json" 0.1.4 || echo "check: refuses 0.1.4"
```

Expected: `latest.json` has `"notes": "Fixed\n- Something a user would notice."` (`$(...)` drops the trailing newline, which is fine), the same `version`, `pub_date` and `platforms` keys as before, and `"changes"` listing 0.1.3, 0.1.2 and 0.1.1. Then `check: accepts 0.1.3` and `check: refuses 0.1.4`.

Note: the first Unreleased `### Fixed` in the copy gets the note, because `replace(..., 1)` touches the first match, and Unreleased is first.

- [ ] **Step 9: Commit**

```bash
git add scripts/release.sh
git commit -m "$(cat <<'EOF'
Take the release notes from the changelog

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: `src/lib/releaseNotes.ts`, test first

**Files:**
- Modify: `tsconfig.json`
- Create: `src/lib/releaseNotes.test.ts`
- Create: `src/lib/releaseNotes.ts`

- [ ] **Step 1: Leave tests out of the app's typecheck**

The app's types have no Node in them (`@types/node` is not installed, on purpose: this is browser code), so `node:test` would not typecheck. In `tsconfig.json`, replace:

```json
  "include": ["src"],
```

with:

```json
  "include": ["src"],
  "exclude": ["src/**/*.test.ts"],
```

- [ ] **Step 2: Write the failing test**

Create `src/lib/releaseNotes.test.ts`:

```ts
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
```

- [ ] **Step 3: Run it to see it fail**

Run: `node --test src/lib/releaseNotes.test.ts`
Expected: FAIL, `Cannot find module '.../src/lib/releaseNotes.ts'`

- [ ] **Step 4: Write `src/lib/releaseNotes.ts`**

```ts
/**
 * What changed, read from the update manifest.
 *
 * `scripts/release.sh` writes a `changes` list beside the manifest's plain
 * `notes`: the new version and the nine before it, newest first. The updater
 * ignores fields it does not know and hands the whole manifest over as
 * `rawJson`, so the notice can show everything since the version you have,
 * not only the newest.
 *
 * Plain values in, nothing imported: Node runs the test for this file as is.
 */

export type VersionNotes = {
  version: string;
  new: string[];
  improved: string[];
  fixed: string[];
};

/** The sections, in the order they are shown. */
export const SECTIONS = [
  ["new", "New"],
  ["improved", "Improved"],
  ["fixed", "Fixed"],
] as const;

const NUMBERS = /^\d+\.\d+\.\d+$/;

/** The versions in `manifest` newer than `current`, newest first. */
export function notesSince(manifest: unknown, current: string): VersionNotes[] {
  const changes = isRecord(manifest) ? manifest.changes : undefined;
  if (!Array.isArray(changes)) return [];
  return changes
    .map(readEntry)
    .filter(
      (entry): entry is VersionNotes =>
        entry !== null && compareVersions(entry.version, current) > 0,
    )
    .sort((a, b) => compareVersions(b.version, a.version));
}

/** Below, at or above zero as `a` is older than, the same as or newer than `b`. */
export function compareVersions(a: string, b: string): number {
  const left = a.split(".").map(Number);
  const right = b.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return 0;
}

function readEntry(raw: unknown): VersionNotes | null {
  if (!isRecord(raw) || typeof raw.version !== "string" || !NUMBERS.test(raw.version)) {
    return null;
  }
  const entry = {
    version: raw.version,
    new: lines(raw.new),
    improved: lines(raw.improved),
    fixed: lines(raw.fixed),
  };
  return entry.new.length + entry.improved.length + entry.fixed.length > 0 ? entry : null;
}

function lines(raw: unknown): string[] {
  return Array.isArray(raw)
    ? raw.filter((line): line is string => typeof line === "string" && line.trim() !== "")
    : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
```

- [ ] **Step 5: Run it to see it pass**

Run: `node --test src/lib/releaseNotes.test.ts`
Expected: `# pass 4` and `# fail 0`

- [ ] **Step 6: Typecheck**

Run: `npx tsc --noEmit && echo types ok`
Expected: `types ok`

- [ ] **Step 7: Commit**

```bash
git add tsconfig.json src/lib/releaseNotes.ts src/lib/releaseNotes.test.ts
git commit -m "$(cat <<'EOF'
Read what changed since the version you have

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: The notice shows it

**Files:**
- Modify: `src/components/UpdateNotice.tsx`
- Modify: `src/styles.css` (after the `.update__text p` rule, around line 2281)
- Modify: `CHANGELOG.md` (this work's own line, under Unreleased)

- [ ] **Step 1: Import the module**

In `src/components/UpdateNotice.tsx`, replace:

```tsx
import { errorMessage } from "../lib/api";
```

with:

```tsx
import { errorMessage } from "../lib/api";
import { SECTIONS, notesSince } from "../lib/releaseNotes";
```

- [ ] **Step 2: Work out the versions to show**

Replace:

```tsx
  if (!update || dismissed) return null;
```

with:

```tsx
  if (!update || dismissed) return null;

  // Everything since the version running, so a skipped release is not a lost
  // one. A manifest without `changes`, a release made by hand say, still has
  // its plain notes.
  const versions = notesSince(update.rawJson, update.currentVersion);
```

- [ ] **Step 3: Draw them**

Replace:

```tsx
      <div className="update__text">
        <b>Version {update.version}</b>
        {update.body && <p>{update.body}</p>}
      </div>
```

with:

```tsx
      <div className="update__text">
        <b>Version {update.version}</b>
        {versions.length > 0
          ? versions.map((entry) => (
              <div key={entry.version} className="update__version">
                {entry.version !== update.version && (
                  <span className="eyebrow">{entry.version}</span>
                )}
                {SECTIONS.map(
                  ([key, label]) =>
                    entry[key].length > 0 && (
                      <div key={key}>
                        <span className="eyebrow">{label}</span>
                        <ul>
                          {entry[key].map((line, index) => (
                            <li key={index}>{line}</li>
                          ))}
                        </ul>
                      </div>
                    ),
                )}
              </div>
            ))
          : update.body && <p>{update.body}</p>}
      </div>
```

- [ ] **Step 4: Styles**

In `src/styles.css`, directly after the `.update__text p { ... }` rule (the one with `white-space: pre-line;`), add:

```css
/* What changed, version by version. The labels are only read, so they are the
   usual mono eyebrow, set tighter; the notes match the plain text above. */
.update__version + .update__version {
  margin-top: 0.875rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--line);
}

.update__text .eyebrow {
  margin: 0.625rem 0 0.25rem;
}

.update__version + .update__version > .eyebrow:first-child {
  margin-top: 0;
}

.update__text ul {
  margin: 0;
  padding-left: 1rem;
  max-width: 34rem;
  list-style: disc;
  font-size: 0.8125rem;
  line-height: 1.55;
  color: var(--muted-2);
  user-select: text;
}
```

- [ ] **Step 5: This work's own line in the changelog**

In `CHANGELOG.md`, under `## Unreleased`, replace the first:

```markdown
### Improved

```

with:

```markdown
### Improved
- The update notice says what is new, what improved and what was fixed, with
  every version since yours when you skipped one.

```

Run: `python3 scripts/changelog.py text Unreleased`
Expected:
```
Improved
- The update notice says what is new, what improved and what was fixed, with every version since yours when you skipped one.
```

- [ ] **Step 6: Typecheck and build**

Run: `npm run build:vite`
Expected: `tsc` passes and Vite prints `✓ built in ...`

- [ ] **Step 7: Look at it once, with a sample update**

`check()` finds nothing to offer while 0.1.2 is the newest release, so the notice is fed a sample for this step only and put back afterwards. The updater's endpoint is not touched.

1. Keep the real file: `cp src/components/UpdateNotice.tsx "/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/UpdateNotice.real.tsx"`
2. In `src/components/UpdateNotice.tsx`, inside the `useEffect`, add this after the `check()` chain. Leave the chain itself in place: it finds nothing while 0.1.2 is the newest release, and removing it would leave `check` unused, which fails the typecheck the Stop hook runs (`noUnusedLocals`).

```tsx
    // PREVIEW ONLY. Put the real file back before committing.
    setUpdate({
      version: "0.1.4",
      currentVersion: "0.1.2",
      body: "Plain notes.",
      rawJson: {
        changes: [
          { version: "0.1.4", new: ["Search inside a transcript."], improved: [], fixed: ["Stopping a job no longer leaves ffmpeg running."] },
          { version: "0.1.3", new: [], improved: ["The update notice says what is new, what improved and what was fixed, with every version since yours when you skipped one."], fixed: [] },
        ],
      },
    } as unknown as Update);
```

3. Start Vite through the owner's dev tool: `node ~/.dev-tools/dev.mjs start --wait 60 -- npm run dev:vite` (it prints JSON with the URL, normally `http://localhost:1420`).
4. With the Playwright MCP. `App.tsx` calls `getCurrentWebview()` in an effect, which reads `window.__TAURI_INTERNALS__` and throws in a plain browser, and with no error boundary React then unmounts everything. So before the page loads, add a stand-in with `browser_run_code_unsafe`:

```js
async (page) => {
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = {
      metadata: {
        currentWindow: { label: "main" },
        currentWebview: { windowLabel: "main", label: "main" },
      },
      invoke: () => Promise.reject(new Error("not inside Tauri")),
      transformCallback: () => 0,
      unregisterCallback: () => {},
      convertFileSrc: (path) => path,
    };
  });
  await page.goto("http://localhost:1420");
}
```

   Then run `document.documentElement.style.background = "#151313"` (in the app the window behind the page is dark glass; a browser shows white), take a screenshot of the notice at the top of the main area, and read the console.
5. Expect: `Version 0.1.4`, then NEW with one bullet and FIXED with one bullet, then a hairline, the label `0.1.3`, and IMPROVED with one bullet. Labels are small, mono, uppercase and dim; bullets are the size and colour of the old notes text. Console errors from the stand-in's rejected calls are expected and not part of this check.
6. If the page still does not render, run `node ~/.dev-tools/dev.mjs stop`, start `npm run dev` through the same tool instead, and ask the owner to look at the window. The preview keeps `check` in use, so the typecheck passes while waiting for the answer.
7. Stop the server: `node ~/.dev-tools/dev.mjs stop`
8. Put the real file back: `cp "/private/tmp/claude-501/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/ae7f2e21-026d-4139-9a4d-39dbc0f01a6c/scratchpad/UpdateNotice.real.tsx" src/components/UpdateNotice.tsx`
9. Run: `grep -n "PREVIEW ONLY" src/components/UpdateNotice.tsx || echo "preview removed"`
   Expected: `preview removed`

- [ ] **Step 8: Commit**

```bash
git add src/components/UpdateNotice.tsx src/styles.css CHANGELOG.md
git commit -m "$(cat <<'EOF'
Say what changed, version by version, before updating

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Write the habit down

**Files:**
- Modify: `CLAUDE.md:156-161` (Updates paragraph) and the Commands list (`CLAUDE.md:214`)
- Modify (outside the repo): `/Users/nemanjamarjanovic/.claude/projects/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/memory/distribution-plan.md`
- Modify: `docs/superpowers/specs/2026-10-23-release-notes-design.md` (status line)

- [ ] **Step 1: The Updates paragraph and the new rules**

In `CLAUDE.md`, replace:

```markdown
**Updates come from GitHub Releases**, out of the public repo `NM193/tzz-app`.
`scripts/release.sh <version> "notes"` sets the version, builds, signs,
publishes, and then checks its own work. `UpdateNotice` asks once at launch and
never again while the app is open: an update that interrupts an hour-long
transcription to announce itself is worse than one that waits until tomorrow.
Only `darwin-aarch64` is published, because the bundled ffmpeg is arm64.
```

with:

```markdown
**Updates come from GitHub Releases**, out of the public repo `NM193/tzz-app`.
`scripts/release.sh <version>` sets the version, builds, signs, publishes, and
then checks its own work. `UpdateNotice` asks once at launch and never again
while the app is open: an update that interrupts an hour-long transcription to
announce itself is worse than one that waits until tomorrow. Only
`darwin-aarch64` is published, because the bundled ffmpeg is arm64.

**What changed is written down as the work happens**, in `CHANGELOG.md`, and
becomes the notes the update notice shows:

- A change someone using the app would notice gets one line under
  `## Unreleased` -- New, Improved or Fixed -- in the same commit as the
  change. Plain English about what they can now do or what stopped going
  wrong, never about files or functions.
- A change nobody using the app would notice -- docs, refactors, scripts,
  tests -- gets no line.
- A release happens only when the owner asks for one. Show what is under
  Unreleased (`python3 scripts/changelog.py text Unreleased`), propose a
  version -- the last number up for fixes and improvements, the middle one
  when anything is new -- and run `bash scripts/release.sh <version>` only once
  the owner has agreed to that version. It pushes and publishes.

`scripts/changelog.py` is the only reader of the file, and a strict one: a
heading other than New, Improved or Fixed stops a release instead of dropping
the notes under it, and a version older than the newest is refused before
anything is pushed. The manifest carries the notes twice: plain `notes`, all
that a copy from before the notice read `changes` can show, and `changes`, the
last ten versions, from which the notice shows everything since yours.
```

- [ ] **Step 2: The commands**

In `CLAUDE.md`, replace:

```markdown
- `cd src-tauri && cargo test` -- 97 tests, all pure logic
```

with:

```markdown
- `cd src-tauri && cargo test` -- 97 tests, all pure logic
- `python3 scripts/test_changelog.py` -- the release notes reader
- `node --test src/lib/releaseNotes.test.ts` -- what the update notice shows
```

- [ ] **Step 3: The project memory**

In `/Users/nemanjamarjanovic/.claude/projects/-Users-nemanjamarjanovic-Desktop-Claude-Projects-yt-mp3/memory/distribution-plan.md`, replace:

```markdown
update; `scripts/release.sh <version> "notes"` builds, signs, publishes and
```

with:

```markdown
update; `scripts/release.sh <version>` (notes from CHANGELOG.md since 2026-10-23) builds, signs, publishes and
```

- [ ] **Step 4: The spec's status**

In `docs/superpowers/specs/2026-10-23-release-notes-design.md`, replace `**Status:** agreed 2026-10-23.` with `**Status:** agreed and built 2026-10-23. The first release to carry it is shown by the old notice.`

- [ ] **Step 5: Everything that has tests**

Run, from the repo root:

```bash
python3 scripts/test_changelog.py
node --test src/lib/releaseNotes.test.ts
npm run build:vite
bash -n scripts/release.sh && echo "release.sh syntax ok"
(cd src-tauri && cargo test 2>&1 | tail -3)
```

Expected: `OK` from Python, `# fail 0` from Node, a clean Vite build, `release.sh syntax ok`, and cargo's `test result: ok` (no Rust was touched; this only proves it).

- [ ] **Step 6: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-23-release-notes-design.md docs/superpowers/plans/2026-10-23-release-notes.md
git commit -m "$(cat <<'EOF'
Write the changelog as the work happens

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
EOF
)"
```
