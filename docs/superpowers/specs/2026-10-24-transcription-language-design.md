# Asking which language is spoken

**Status:** designed 2026-10-24, not built.

## What was found

A recording of the owner speaking Serbian over music came back as English
nonsense: "I'm talking about music and music. I'm talking about 20 seconds."
Local files and recordings always run Whisper with `-l auto` -- `useJobs`
passes `language: null` -- and Whisper guesses from the first 30 seconds.

Measured on 2026-10-24 with the owner's `ggml-large-v3-turbo.bin` and the
bundled `whisper-cli`:

| Audio | `-l auto -dl` says | `-l auto` writes | `-l sr` writes |
|---|---|---|---|
| Serbian speech, nothing else | `hr`, p 0.53 | correct Latin Serbian | the same, numbers as words |
| Serbian speech over music | `en`, p 0.38 | English nonsense | rough but real Serbian |

- Detection alone (`-dl`) takes about a second.
- `-l hr` and `-l bs` drift into ijekavian ("ovdje"); `-l sr` stays ekavian
  and, on both samples, Latin.
- **Translation is out of reach**, and was set aside by the owner: turbo
  ignores `-tr` (it returned the Serbian unchanged), Whisper only ever
  translates into English, and Apple's on-device Translation framework on
  macOS 26.5 has no Serbian, Croatian or Bosnian in either direction. A
  translation goes through a chapter's Copy and comes back as a note, as now.

So the guess cannot simply be shown and accepted: here it was the wrong one.
It has to be shown and correctable.

## Decided

Before a recording or a local file is transcribed, a dialog shows the
language Whisper hears, already chosen, and lets the owner change it. YouTube
links are untouched: their Whisper fallback already takes the Caption
language setting.

### What you see

```
Which language is spoken?

Recording 2026-10-24 14-50.mp3
DETECTED · ENGLISH 38%           [ Serbian ] [•English•]

Lecture 3.mp4
LISTENING…                       [ Serbian ] [ English ]

                                   Cancel     [Transcribe]
```

- **When:** every time a file is about to join the queue -- a recording that
  just stopped, a file dropped on the window, Choose files, and Transcribe on
  a saved recording. Today all four call `jobs.enqueue([fileItem(path)])`
  directly.
- **Several files** share one dialog, a row each. A file that arrives while
  the dialog is open -- a second recording stopping, another drop -- is added
  to it as a new row, never a second dialog.
- **Each row starts at `LISTENING…`** and, a second or three later, reads
  `DETECTED · CROATIAN 53%` and selects the language. A choice made before
  that wins; the detection does not overwrite it.
- **Choices:** Serbian and English. `hr`, `bs` and `sr` all select Serbian,
  which sends `sr`. Any other language Whisper names gets a third button of
  its own, named in English (`Intl.DisplayNames`).
- **When detection fails** -- no model chosen, no sound, an error -- the row
  reads `NOT DETECTED` and selects the Caption language setting
  (`transcriptLangs[0]`). No new setting. The transcription then reports its
  own error as it does today, for instance that no model is chosen.
- **Cancel** transcribes nothing. A recording stays saved where it is.
- **Transcribe waits** until every row has a guess or a choice made by hand,
  so Enter cannot send a file off on the fallback while Whisper is still
  listening. A guess that has not come back after 60 s is given up on and the
  row reads `NOT DETECTED`.
- **Transcribe** queues every row with its language. The transcript's header
  then says `Transcript from: whisper (sr)`, not `whisper (auto)`; the header
  already prints whatever language it is given.

The dialog follows `docs/design-reference-work.md`: the question in the serif,
file names and buttons in the sans, `DETECTED · …` in mono, uppercase and dim,
since it is read, not clicked. It opens with the same wipe from below as the
cards.

### How it works

**Rust, `services/language.rs`** (new):

1. Read the file's duration from ffmpeg's header (`Duration: 00:12:34.56`),
   by running `ffmpeg -i <file>` with no output and reading its stderr. That
   call always exits non-zero. A file without a `Duration:` line, or with
   `Duration: N/A`, is heard from the start.
2. Cut 30 s, 16 kHz mono, starting a third of the way in, so a lecture's
   intro music does not decide; a file of 30 s or less is taken from the
   start. The excerpt goes to a uniquely named file in the temp folder.
3. Run `whisper-cli -m <model> -f <excerpt> -l auto -dl` and read
   `auto-detected language: hr (p = 0.526195)` from its stderr, where
   whisper.cpp logs it.
4. Delete the excerpt, whatever happened.

It returns `{ code, probability }` and nothing else; naming and grouping
languages is the dialog's business. It does **not** register with
`services/cancel.rs`: Stop on a running job must not kill a detection, nor a
detection be mistaken for the job. Its processes are `kill_on_drop`.

`ffmpeg` and `whisper-cli` are resolved through `services/binaries.rs`, as
everywhere else.

Three pure functions carry the logic and get tests: reading the duration,
choosing where the excerpt starts, and reading Whisper's detection line.

**Command:** `detect_language(path, whisperModelPath)` in
`commands/media.rs`, validation only -- the same path validation
`transcribe_file` uses, and a missing model is an error.

**Frontend:**

- `api.ts`: `detectLanguage(path, modelPath)` and `DetectedLanguage
  { code, probability }`, mirroring the Rust struct (camelCase).
- `queue.ts`: `QueueItem` gains `language?: string | null`.
- `useJobs.ts`: `runOne` sends `item.language ?? null` instead of `null`.
- `LanguagePrompt.tsx` (new): the dialog. It detects one file at a time, so
  two Whisper processes never compete for the GPU and memory.
- `App.tsx` owns the dialog's state and hands `ConvertView` and `RecordView`
  one function to transcribe paths, in place of their own
  `jobs.enqueue([fileItem(path)])`.
- `styles.css`: the dialog, from the existing tokens.

A detection may run while another job transcribes. That is two models in
memory for a few seconds; accepted.

## Not in scope

- Translation, in either direction (see above).
- A remembered last choice. The detection, corrected when wrong, is the
  default.
- YouTube links.
- The status line still says "Files are in the library" after a local file,
  whose transcript is written beside it. A separate fix.

## Done when

- `cargo test` covers the three pure functions; `npm run build:vite` is clean.
- On the installed app: the owner's music-over-speech recording, confirmed as
  Serbian in the dialog, comes back in Serbian with `whisper (sr)` in its
  header; a clean Serbian recording arrives with Serbian already selected.
- `CHANGELOG.md`, under Unreleased, New: "Before transcribing a recording or
  a file, the app asks which language is spoken, with its own guess already
  chosen."
- `CLAUDE.md` lists `language` among the services.
