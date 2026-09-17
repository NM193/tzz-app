# Tzz App

## What this is
A macOS desktop app that turns lectures into readable material. It takes
YouTube links or local audio and video, and produces an MP3, a timestamped
transcript, and -- for screen-share videos -- the text that was on screen.
It can also record an audio input directly.

Single-purpose personal tool: no accounts, no server, no database.

The crate and folder are still called `yt-mp3`; only the product name changed.
The bundle identifier `com.nemanja.ytmp3` is deliberately unchanged, because
macOS ties permissions and settings to it.

## Tech stack
- Tauri v2 (Rust backend, WKWebView frontend)
- React 18 + TypeScript + Vite
- External CLIs: yt-dlp, ffmpeg, whisper-cli (whisper.cpp)
- `genpdf` for PDF output
- A Swift OCR helper using Apple's Vision, compiled by `build.rs` and embedded
- Hand-written CoreAudio FFI for switching the sound output

**`swiftc` is required to build** (`xcode-select --install`).

## Project structure
- `src/` React frontend. `src/lib/api.ts` is the only file that calls `invoke`.
  `lib/useJobs.ts` and `lib/useRecorder.ts` hold state; `components/*View.tsx`
  are the four screens behind the sidebar.
- `src-tauri/src/commands/` thin Tauri command handlers, validation only.
- `src-tauri/src/services/` all real logic:
  - `binaries` finding external tools · `ytdlp` downloading · `transcript`
    parsing and formatting · `screen` reading text off video · `pdf` ·
    `combine` stitching a queue into one document · `recorder` ·
    `audio_output` CoreAudio · `tray` the menu bar icon
- `src-tauri/helpers/ocr.swift` the Vision text reader.

## Key patterns
- Commands are thin, services hold the logic. Do not put process spawning in a
  command handler.
- Rust structs facing the frontend use `#[serde(rename_all = "camelCase")]`.
  `src/lib/api.ts` mirrors them by hand -- change one, change the other.
- External binaries are resolved through `services/binaries.rs`, never via a
  bare `Command::new("name")`. A Finder-launched app has no Homebrew PATH; the
  app really does run with `PATH=/usr/bin:/bin:/usr/sbin:/sbin`.
- Progress is emitted on the `download://progress` channel. Emit failures are
  ignored on purpose: progress must never abort a job.
- User-facing error strings are plain sentences, not debug output.
- Captions and Whisper both produce SRT so there is exactly one formatting
  path. Do not add a second one that skips the timings.

## Lessons the code encodes
These were bugs. The comments in the code say the same; this is the index.

- **Prefer the original caption track.** YouTube offers machine translations
  next to the real thing. Translating a machine transcript compounds the
  errors: on an English course, Montserrat came back as "Монзерат".
- **Everything repeats.** Auto-captions scroll, so each cue restates the last
  line of the one before. Application chrome is identical in every frame. Both
  need "show only what is new", or the output triples in size and says less.
- **Screens belong to exactly one paragraph.** Paragraphs break at chapter
  boundaries as well as minutes, so a fixed 60-second window double-counts.
- **A killed app orphans ffmpeg.** The recorder writes its pid beside the
  recording; the next launch stops what was left running and puts the sound
  output back.
- **OCR is cached** in `screens.json` beside the video. Reading an hour costs
  twelve minutes and never changes.
- **`--no-part` plus an existing file is a trap.** yt-dlp treats a finished
  download as an interrupted one and fails resuming past its end (HTTP 416).
  Check for the file first.

## Working on this
- **Do not edit Rust while a job is running.** The dev watcher restarts the app
  on every change, which kills the job and orphans its child processes.
- Output goes to `~/Documents/Tzz Library`, one folder per video.
- The window is transparent and frosted by macOS (`services/glass.rs`), so the
  stylesheet paints tints, never an opaque background.
- Stopping a job (`services/cancel.rs`) kills its process group and deletes
  what it wrote. One job runs at a time, so this is a global, on purpose.

## Moving to another Mac
No updater and no signing: this app lives on one machine. To move it, copy
the folder (or `git clone`), then `brew install yt-dlp ffmpeg whisper-cpp`,
`xcode-select --install` for `swiftc`, install BlackHole for system audio,
drop a Whisper `ggml-*.bin` model where Settings can find it, and run
`npm run install:app`.

## Current status
Working: single video and queue, MP3, transcript from captions or Whisper,
screen text via OCR, chapters and a table of contents, markdown and PDF, one
combined document per queue, local audio and video files, audio recording with
a menu bar icon and level meter, automatic output-device switching, stopping a
job mid-run, a Library screen listing the output folder.

Not built: summaries, courses and quizzes (see
`docs/superpowers/specs/2026-08-30-study-material-vision.md`), playlists,
Windows support.

## Commands
- `npm run dev` -- run the app (Vite + Tauri)
- `npm run build` -- bundle a .app / .dmg
- `npm run install:app` -- build and replace the copy in /Applications
- `npm run build:vite` -- typecheck the frontend only
- `cd src-tauri && cargo test` -- 63 tests, all pure logic
