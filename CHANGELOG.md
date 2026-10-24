# Changelog

What changed in each version of Tzz App, newest first. The app shows these
notes when it offers an update, so each line is written for the person using
it, not about the code.

## Unreleased

### New

### Improved

### Fixed

## 0.1.3 — 2026-10-24

### Improved
- The update notice says what is new, what improved and what was fixed, with every version since yours when you skipped one.

### Fixed
- The app opens on a Mac it was not built on. macOS asks once, in System Settings under Privacy & Security, instead of calling the app damaged.
- Transcribing with Whisper and reading text off the screen work on any Apple Silicon Mac with macOS 13.3 or later, not only on the newest ones.

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
