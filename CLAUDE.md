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
- Hand-written CoreAudio FFI for switching the sound output and reading inputs

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
    kept per chapter · `recorder` · `capture` reading an input ·
    `audio_output` CoreAudio · `tray` the menu bar icon · `cancel` stopping a
    job and deleting what it wrote · `library` reading the output folder ·
    `thumb` one picture per folder · `glass` the frosted window ·
    `model` fetching a Whisper model · `language` hearing which language a
    file is in · `mp3` an MP3 of a file on disk · `names` what a job's
    folder is called, and whether the library already has it
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
- **A killed app used to orphan ffmpeg.** Now ffmpeg reads from pipes that
  close with the app, so it finishes the file and exits by itself. The pid
  written beside the recording is a backstop: the next launch stops that
  process only if it is still ffmpeg writing into the recording folder, since
  by then the number may belong to anything, and puts the sound output back.
- **ffmpeg's avfoundation input drops audio.** It holds one buffer and looks
  for the next every 10 ms; at 48 kHz CoreAudio delivers one every 10.7 ms,
  and about one in ten was overwritten unread -- 6-10% of every recording,
  in 512-sample slices. The app reads inputs itself with an AudioQueue and
  hands ffmpeg plain samples.
- **BlackHole runs at the Multi-Output Device's rate, or the recording is
  garbage.** A Multi-Output Device does not set the rate of the devices in
  it, and drift correction cannot bridge 44.1 to 48 kHz: the recording came
  out 9% fast, twice as loud and torn. The app sets the input's rate before
  it starts recording.
- **OCR is cached** in `screens.json` beside a downloaded video, or in a
  file's library folder. Reading an hour costs twelve minutes and never
  changes.
- **A playlist link is `/playlist?list=`, and nothing else.** A watch link
  carries a `list` too -- that is the video you clicked from inside a playlist.
  Unpacking those would turn one pasted link into forty pills. The listing is
  read with `--flat-playlist`, one request for the whole thing, which also
  supplies every title: reading them one by one is how you earn an HTTP 429.
- **`--no-part` plus an existing file is a trap.** yt-dlp treats a finished
  download as an interrupted one and fails resuming past its end (HTTP 416).
  The video checks for the file first. The audio passes `--no-continue`,
  because a run that failed at the conversion leaves its finished download
  behind; a finished MP3 is still recognised and kept.
- **yt-dlp looks for ffmpeg only on PATH.** Resolving ffmpeg ourselves is not
  enough: opened from Finder, yt-dlp saw no ffmpeg at all -- not Homebrew's,
  not the one beside it inside the app -- and every download failed at the MP3
  conversion. A yt-dlp run that converts anything gets `--ffmpeg-location`.
  `tauri dev` never shows this, because the terminal's PATH has Homebrew.
- **Every folder can show a picture, one way or another.** `thumb.rs` tries the
  cover yt-dlp wrote, then a frame from 30 seconds in (not the first second --
  that is a title card), then the waveform of the audio, then the cover fetched
  from YouTube using the id in the folder's own name. It is cached as
  `poster.jpg` and never recomputed.

## Working on this
- **Do not edit Rust while a job is running.** The dev watcher restarts the app
  on every change, which kills the job and orphans its child processes.
- Output goes to `~/Documents/Tzz Library`, one folder per video or file. A
  file on disk is never touched and nothing is written beside it: its
  transcript, MP3, picture and screens cache go into its library folder.
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
- **The app is signed ad hoc, not by Apple, and on macOS 15 and later that is
  not a right-click away.** Apple removed the Control-click override: the
  first open is refused outright, and the way through is System Settings ->
  Privacy & Security, where a line about the blocked app appears with **Open
  Anyway**. Only the first open, and only for a copy that came down through a
  browser. Writing "right-click -> Open" in the instructions sends someone to
  a menu item that no longer helps.
- **Ad hoc is not optional.** `bundle.macOS.signingIdentity` is `"-"`. Without
  it the bundle carries only the linker's signature, which covers the binary
  and not the bundle, and a downloaded copy is "damaged" with nothing offered
  in Privacy & Security -- 0.1.2 and earlier shipped that way. Hardened
  runtime is off: without notarization it buys nothing, and with it the
  microphone needs an entitlement, for the app and for the ffmpeg it records
  through.
- **The oldest macOS is one number**, `bundle.macOS.minimumSystemVersion`
  (13.3). Tauri writes it into `Info.plist` and builds the Rust for it.
  `build.rs` passes it to `swiftc` and `fetch-tools.sh` to cmake, because
  neither reads it on its own, and a tool built here without it asks for this
  Mac's macOS: 0.1.2's whisper-cli and OCR helper asked for 26. It is 13.3,
  not 13.0, because ggml's BLAS backend calls the `cblas_sgemm` interface that
  arrived in 13.3. `release.sh` checks the signature and every binary against
  the number before it publishes.
- **whisper-cli is built for every Apple Silicon chip** (`GGML_NATIVE=OFF`).
  Left to itself, ggml compiles its CPU kernels for the Mac that builds them:
  on this M5 that put 284 i8mm instructions into 0.1.2's whisper-cli, and an
  M1 has none. A whisper-cli built natively still answers for itself, so
  `fetch-tools.sh` keeps it: `--force`, or delete it, after changing a flag.

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
the notes under it, and a version typed by mistake -- an older one, or the
current one while Unreleased has notes -- is refused before anything is pushed.
The manifest carries the notes twice: plain `notes`, all that a copy from
before the notice read `changes` can show, and `changes`, the last ten
versions, from which the notice shows everything since yours.

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
unpacking a playlist link into the videos it holds, asking which language a
file is spoken in before transcribing it, finding a word in an open
document, copying a chapter out and keeping what you bring back as a note,
and asking what to do when a job's name is already in the library.

Not built: Windows support.

## Commands
- `npm run dev` -- run the app (Vite + Tauri)
- `npm run build` -- bundle a .app / .dmg
- `npm run install:app` -- build and replace the copy in /Applications
- `npm run build:vite` -- typecheck the frontend only
- `bash scripts/fetch-tools.sh` -- put the external tools inside the app
- `cd src-tauri && cargo test` -- 142 tests, all pure logic
- `python3 scripts/test_changelog.py` -- the release notes reader
- `node --test src/lib/releaseNotes.test.ts` -- what the update notice shows
- `node --test src/lib/naming.test.ts` -- what the names dialog asks and refuses
