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
    parsing and formatting · `document` parsing one back for the reader ·
    `screen` reading text off video · `pdf` ·
    `combine` stitching a queue into one document · `notes` your own notes,
    kept per chapter · `recorder` ·
    `audio_output` CoreAudio · `tray` the menu bar icon · `cancel` stopping a
    job and deleting what it wrote · `library` reading the output folder ·
    `thumb` one picture per folder · `glass` the frosted window ·
    `model` fetching a Whisper model
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
- `transcript.rs` writes the document and `document.rs` reads it back. They are
  one format and belong together: change the shape in one, change both, and the
  tests in `document.rs` will say so.

## Design
The look is documented in `docs/design-reference-work.md`, extracted from
another project of the owner's. Two rules carry it:

- **One accent, three greys, hairlines.** Every colour lives at the top of
  `src/styles.css` and nowhere else. `--muted` is for large text only.
- **Two faces, hard split.** The serif is for headings only; the sans is the
  interface; mono is for data and labels. **Anything clickable is sans and
  brighter; anything you only read is mono, uppercase and dim.** That is how a
  row's status never gets mistaken for one of its buttons.

Motion is one gesture repeated: a wipe from below. Cards open bottom to top,
rules draw from the left. Script-driven animation checks
`prefers-reduced-motion` itself -- the global CSS rule does not reach it.

Settings chooses the window's surface (solid, glass, or an aurora the app
paints itself), its tint, and the accent. There is no light theme: this palette
has no light version, and one would be a different design.

The mark is sound on the left and writing on the right -- the job, drawn.
`scripts/make-icon.swift` is its only source: it draws the icon and the menu
bar images with CoreGraphics (swiftc is required anyway, for the OCR helper),
and `npx tauri icon assets/icon.png` turns the result into every size macOS
wants. Menu bar images are templates -- black on nothing, recoloured by macOS --
and use a simpler version of the mark, because the full one closes up below
about 32 points.

Prototypes live in `.superpowers/prototype/` and open with a double click.

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
- **A playlist link is `/playlist?list=`, and nothing else.** A watch link
  carries a `list` too -- that is the video you clicked from inside a playlist.
  Unpacking those would turn one pasted link into forty pills. The listing is
  read with `--flat-playlist`, one request for the whole thing, which also
  supplies every title: reading them one by one is how you earn an HTTP 429.
- **`--no-part` plus an existing file is a trap.** yt-dlp treats a finished
  download as an interrupted one and fails resuming past its end (HTTP 416).
  Check for the file first.
- **Every folder can show a picture, one way or another.** `thumb.rs` tries the
  cover yt-dlp wrote, then a frame from 30 seconds in (not the first second --
  that is a title card), then the waveform of the audio, then the cover fetched
  from YouTube using the id in the folder's own name. It is cached as
  `poster.jpg` and never recomputed.

## Working on this
- **Do not edit Rust while a job is running.** The dev watcher restarts the app
  on every change, which kills the job and orphans its child processes.
- Output goes to `~/Documents/Tzz Library`, one folder per video.
- The window is transparent and frosted by macOS (`services/glass.rs`), so the
  stylesheet paints tints, never an opaque background.
- Stopping a job (`services/cancel.rs`) kills its process group and deletes
  what it wrote. One job runs at a time, so this is a global, on purpose.

## Giving it to someone else
**The app carries its own tools.** `scripts/fetch-tools.sh` puts `yt-dlp`,
`ffmpeg` and `whisper-cli` in `src-tauri/binaries/`, and `bundle.externalBin`
copies them into the `.app`. 88 MB of them, so they are not in git: run the
script once on a fresh clone, before `npm run build`.

- **yt-dlp** and **ffmpeg** are fetched as the portable builds their authors
  publish. Homebrew's ffmpeg links nineteen Homebrew libraries and its yt-dlp
  is a shell script pointing into the Cellar; neither can be carried over.
- **whisper-cli is built from source**, with the libraries, the backends and
  the Metal shaders inside it. Copying Homebrew's does not work: it loads its
  backends from a path compiled into it, and `GGML_BACKEND_PATH` can only add
  one file, so on a Mac without Homebrew it comes up with no CPU and no Metal.
  `cmake` is needed to build it.
- **The Whisper model cannot travel** -- the one worth having is 1.5 GB. The
  Settings screen offers three and fetches one (`services/model.rs`).
- **BlackHole still has to be installed by hand.** Its GitHub releases carry no
  files; the installer is behind a form on the authors' site. Record notices
  when no loopback device exists and offers the page; the URL lives in Rust, so
  nothing on screen can decide where the app sends you. Only needed to record
  the Mac's own sound.
- **The app is unsigned, and on macOS 15 and later that is not a right-click
  away.** Apple removed the Control-click override: the first open is refused
  outright, and the way through is System Settings -> Privacy & Security, where
  a line about the blocked app appears with **Open Anyway**. Only the first
  open, and only for a copy that came down through a browser. Writing
  "right-click -> Open" in the instructions sends someone to a menu item that
  no longer helps.

**Updates come from GitHub Releases**, out of the public repo `NM193/tzz-app`.
`scripts/release.sh <version> "notes"` sets the version, builds, signs,
publishes, and then checks its own work. `UpdateNotice` asks once at launch and
never again while the app is open: an update that interrupts an hour-long
transcription to announce itself is worse than one that waits until tomorrow.
Only `darwin-aarch64` is published, because the bundled ffmpeg is arm64.

The key lives at `~/.tauri/tzz.key`, outside the repo, and has no password, so
a release needs nothing interactive. **Never read it** -- only `tzz.key.pub`,
and only to fill `plugins.updater.pubkey`. The app trusts that one key and no
other, so replacing the key after a release breaks every installed copy.

Three things here were bugs, and the script now proves each one rather than
assuming it:

- **GitHub renames assets.** `Tzz App.app.tar.gz` is served as
  `Tzz.App.app.tar.gz`. A url built from the filename 404s, and the app
  announces an update it cannot fetch. The name is read back from GitHub.
- **`releases/latest` lags by seconds.** Checking it straight after publishing
  validates the *previous* release's manifest and passes.
- **A signature belongs to particular bytes.** The archive, its signature and
  the manifest are replaced together or not at all.

## What this is for
The point is not to answer questions about a lecture -- **NotebookLM does that
for free**, and the owner uses it. The point is to hand it a better source than
it can gather itself: given a YouTube link it reads only the captions and never
sees the picture, while this app's markdown carries timestamps, chapters, a
table of contents and the text that was on screen.

So effort goes into the quality of the document, not into a model. Summaries,
courses and quizzes are **deliberately not built** (see the spec above).

A local model was built and removed on 2026-09-28. It worked -- gemma3:27b
wrote accurate Serbian from an English lecture in about half a minute a
chapter -- but it was incomplete in ways a prompt could not fix, and it decided
for the owner which model was worth using. What replaced it is smaller and
better: each chapter has a **Copy**, and whatever you bring back is kept as a
note against that chapter. The app carries the text; you choose what reads it.

## Current status
Working: single video and queue, MP3, transcript from captions or Whisper,
screen text via OCR, chapters and a table of contents, markdown and PDF, one
combined document per queue, local audio and video files, audio recording with
a menu bar icon and level meter, automatic output-device switching, stopping a
job mid-run, a Library that searches and filters what is on disk, a picture per
folder, a notification when a run ends, reading a transcript inside the app,
unpacking a playlist link into the videos it holds, finding a word in an open
document, copying a chapter out and keeping what you bring back as a note.

Not built: Windows support.

## Commands
- `npm run dev` -- run the app (Vite + Tauri)
- `npm run build` -- bundle a .app / .dmg
- `npm run install:app` -- build and replace the copy in /Applications
- `npm run build:vite` -- typecheck the frontend only
- `bash scripts/fetch-tools.sh` -- put the external tools inside the app
- `cd src-tauri && cargo test` -- 97 tests, all pure logic
