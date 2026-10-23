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
