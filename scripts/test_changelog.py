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
