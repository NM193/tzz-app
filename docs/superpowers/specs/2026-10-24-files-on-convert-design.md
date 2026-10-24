# Files on Convert wait for Start, and go to the library

**Status:** designed 2026-10-24, the day 0.2.0 shipped; not built.

## What was found

The owner's wife used 0.2.0 on her own Mac:

1. **A dropped video started at once.** The language dialog opened, and
   Transcribe sent the file off before she could tick Read the screen or
   anything else. This predates the dialog -- a dropped file always went
   straight into the queue -- but the dialog made it look like a
   confirmation step that skipped the options.
2. **"Save to: Tzz Library" was ignored.** Her transcript landed in
   `~/Documents/Snimci`, beside the video, and the library stayed empty. A
   local file's transcript has always been written beside it
   (`transcript_path_for`); `LocalFileRequest` never carried a folder. The
   app also writes a `poster.jpg` beside the original (the owner's
   `~/Desktop/Andjela MP3 Leaning/poster.jpg`, 2026-09-28) and the OCR cache
   (`<video>.screens.json`). The status line meanwhile says "Files are in
   the library".

## Decided (by the owner)

- **Convert:** a dropped or chosen file becomes a pill beside the links and
  waits for Start, with the options below applied to it. A file dropped on
  any screen does the same, after switching to Convert.
- **Record keeps the language dialog**, after Stop and on Transcribe.
- **Language moves to Convert**, in the Transcript group: Serbian | English,
  the language spoken. It is the existing `transcriptLangs` setting: for a
  link it picks the caption track and Whisper's language, for a file it is
  Whisper's language. "Caption language" leaves Settings.
- **Everything a file makes goes into the Save to folder**, one folder per
  file like a download: the transcript, its picture, the OCR cache, and an
  MP3 when Audio is ticked. **The original is never touched and nothing is
  written beside it.**

## How

**The folder** is `<Save to>/<file name without extension>`, sanitised like
a download's (`combine::sanitise`, 120 characters), claimed for Stop before
it is made. Transcribing the same file again uses the same folder and
replaces what is in it, as a download does. Two different files with the
same name share a folder; accepted.

**The options, for a file:**

| Option | Applies? |
|---|---|
| Audio | yes: an MP3 in the folder. An MP3 is copied, not re-encoded; anything else is encoded at the download quality (`v0` → `-q:a 0`, `320` → `-b:a 320k`). |
| Transcript | yes. Off means no transcript -- except from Record, whose button says Transcribe, so the dialog forces it on. |
| Markdown / PDF / Read the screen | yes, as for a download. |
| Video file | no: the original is already kept, where it is. |
| Language | yes, unless the Record dialog chose one. |

Audio and Transcript both off is an error, as for a link.

**The picture:** a frame of the original if it is a video, else its
waveform, else whatever the folder offers (`thumb::ensure_from`).

**The OCR cache** goes in the folder: `read_screens` takes the cache path
instead of deriving it from the video. A download keeps its cache where it
was.

**Code:** a new `services/mp3.rs` (copy or encode, tracked by `cancel`);
`thumb::ensure_from`; `screen::read_screens(app, video, cache, work_dir)`;
`LocalFileRequest` gains `outDir`, `wantAudio`, `wantTranscript`,
`audioQuality`; `job_folder` is split so a file's folder is made the same
way. Frontend: `PendingLink.file`, `pendingFile(path)`; `ConvertView` maps
file pills to file jobs at Start; the language Segmented joins the
Transcript group; `QueueItem.wantTranscript` carries the Record dialog's
"always".

## Not in scope

- Moving a recording into the library: it stays where it was saved; with
  Audio ticked, its folder gets a copy.
- Per-file language on Convert: one language per Start.

## Done when

- `cargo test` covers the MP3 decisions; `npm run build:vite` is clean.
- On the installed app: a video dropped on Convert waits as a pill; Start
  with Read the screen ticked produces `Tzz Library/<name>/` holding the
  transcript, an MP3, `poster.jpg` and the screens cache, and nothing new
  appears beside the original. A recording transcribed from Record lands in
  the library too.
- `CHANGELOG.md` Unreleased: one Improved, one Fixed line.
- `CLAUDE.md` lists `mp3` among the services; the memory
  `local-audio-transcription` is corrected.
