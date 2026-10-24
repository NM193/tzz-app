# Changelog

What changed in each version of Tzz App, newest first. The app shows these
notes when it offers an update, so each line is written for the person using
it, not about the code.

## Unreleased

### New
- Settings shows which version you have, and Check for updates looks for a
  newer one whenever you like.

### Improved
- A file dropped on Convert waits for Start like a link, so you can set its
  options first, the language spoken among them.

### Fixed
- A file or recording you transcribe now goes into the folder you save to, in
  a folder of its own with an MP3, instead of beside the original.

## 0.2.0 — 2026-10-24

### New
- Before transcribing a recording or a file, the app asks which language is spoken, with its own guess already chosen.

### Fixed
- Recording the Mac's sound no longer comes out too fast and garbled when BlackHole and the Multi-Output Device run at different sample rates: the app matches them before it starts.
- Recordings no longer lose a sliver of sound about ten times a second.
- Clicking the app in the Dock brings its window back after it was minimized or closed.

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
