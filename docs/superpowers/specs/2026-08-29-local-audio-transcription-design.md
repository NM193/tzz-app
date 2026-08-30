# Local audio file transcription

**Status:** approved 2026-08-29

## Problem

The app only accepts YouTube URLs. A local recording -- an interview, a
podcast episode already on disk, an `.m4a` from a phone -- cannot be
transcribed without first uploading it somewhere. Whisper is already wired
up and running locally; only the input path is missing.

## What we build

A second way into the existing transcription pipeline: drop an audio file on
the window (or pick it from Finder) and get a timestamped markdown transcript
saved next to it.

Out of scope: converting local files to MP3, batch/queue processing, editing
the transcript in-app.

## Key insight

`transcript::whisper_srt()` already takes an arbitrary audio path and pipes it
through ffmpeg into 16 kHz mono PCM before calling whisper-cli. No new
transcription logic is needed -- only an entry point that skips yt-dlp.

This preserves the invariant from CLAUDE.md: captions and Whisper both produce
SRT, so there is exactly one formatting path. The local-file flow reuses it
whole.

## Backend

New command `transcribe_file` in `commands/media.rs`, thin like the others:

    LocalFileRequest { path, whisperModelPath, language? }
      -> validate: file exists, extension supported, model set, whisper-cli found
      -> emit ProgressEvent::stage("transcribing", ...)
      -> transcript::whisper_srt(path, model, language.unwrap_or("auto"))
      -> parse_srt -> cues_to_markdown -> save_markdown
      -> emit ProgressEvent::stage("done", None)
      -> JobResult

Registered in `lib.rs` alongside the existing handlers.

### Return type

Reuses the existing `JobResult` rather than introducing a new struct:

- `title` -- the file stem
- `audioPath` -- the source file path
- `transcript` -- the result
- `warnings` -- empty in the happy path

The frontend result panel, "Copy", and "Reveal in Finder" therefore work
unchanged. A new result type would have forced parallel UI for no gain.

### Progress

Emitted on the existing `download://progress` channel using existing `Stage`
values (`transcribing`, `done`). No new stages, no new channel. Emit failures
stay ignored, as everywhere else.

### Language

Defaults to `auto`; whisper-cli detects the language itself. Verified working
with `ggml-large-v3-turbo` on this machine. The `language` field stays on the
request so a future UI override needs no backend change, but nothing sets it
today.

### Accepted extensions

`mp3 m4a wav flac ogg opus aac mp4 mov mkv webm`

Everything ffmpeg handles in practice. Anything else is rejected up front with
a plain sentence, not a debug dump.

### Guard clauses

Both checks run before any work starts, so the job cannot die halfway:

1. `whisper-cli` resolvable via `binaries::resolve` -- otherwise
   "whisper-cli is not installed. Install it with: brew install whisper-cpp"
2. A whisper model path is set and points at a real file -- otherwise
   "Set a Whisper model path in Settings first."

## Not overwriting the user's files

`save_markdown` writes `<stem>.md` next to the audio. For a YouTube download
that file was just created, so overwriting is harmless. For a file already on
disk it is not: `interview.mp3` would clobber an existing `interview.md`.

Local transcription therefore picks a non-destructive path: if `<stem>.md`
already exists, write `<stem>.transcript.md` instead. The YouTube flow keeps
its current behaviour.

This is a small pure function, `transcript_path_for(audio_path)`, so it can be
tested without touching the filesystem beyond an existence check.

## Frontend

`src/lib/api.ts` gains `LocalFileRequest` and `transcribeFile(request)`,
mirroring the Rust struct by hand as the other types do.

`src/App.tsx` gains a dropzone below the URL input: "Prevuci audio fajl ovde
ili izaberi", with a visual state while a file is dragged over it. Drag and
drop uses Tauri v2's `onDragDropEvent`; the button reuses the same `open()`
dialog `pickModel` already uses.

The dropzone is disabled while a job is running, matching the URL input.

## Testing

`cargo test`:
- extension validation accepts the listed formats and rejects others
- `transcript_path_for` returns `<stem>.md` when free, `<stem>.transcript.md`
  when taken

`npm run build:vite` typechecks the frontend.

Manual: drop a local MP3, confirm the markdown lands next to it with timings.
