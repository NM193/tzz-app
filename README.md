# YT MP3

A small macOS desktop app: paste a YouTube link, get an MP3 in `~/Downloads` and,
when it's available, a plain-text transcript next to it.

Tauri v2 + React + TypeScript on the front, Rust orchestrating `yt-dlp`,
`ffmpeg` and optionally `whisper.cpp` on the back.

---

## Setup

```bash
# 1. External tools (the app shells out to these, it does not bundle them)
brew install yt-dlp ffmpeg

# 2. Project
npm install

# 3. Generate the full icon set (.icns / .ico) from assets/icon.png
npm run tauri icon assets/icon.png

# 4. Run
npm run dev
```

Build a distributable `.app` / `.dmg`:

```bash
npm run build
```

Keep `yt-dlp` current — YouTube changes break older versions regularly:

```bash
brew upgrade yt-dlp
```

---

## Optional: Whisper for videos without captions

YouTube has captions for most English content but rarely for Serbian. When a
video has none, the app can transcribe the MP3 locally instead.

```bash
brew install whisper-cpp

# Pick a model. large-v3-turbo is the best quality/speed trade-off on Apple silicon.
mkdir -p ~/whisper-models && cd ~/whisper-models
curl -L -O https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin
```

Then open **Options** in the app and point *Whisper model* at that `.bin` file.
Smaller models (`ggml-small.bin`, ~460 MB) are much faster but noticeably worse
at Serbian.

---

## Output

Every run writes two files side by side in the output folder:

```
Some Video Title [dQw4w9WgXcQ].mp3
Some Video Title [dQw4w9WgXcQ].md
```

The `.md` carries a metadata header (title, channel, source URL, duration,
where the transcript came from) followed by one timestamped paragraph per
minute:

```markdown
**[04:00]** ...one minute of speech...

**[05:00]** ...the next minute...
```

The timestamps are the reason the pipeline keeps SRT all the way through
instead of flattening to plain text early -- a passage can always be traced
back to a spot in the video.

### MP3 quality

Two options, set in **Options**:

- **VBR V0** (default) -- LAME variable bitrate, ~245 kbps average.
- **CBR 320** -- constant 320 kbps, bigger files.

Worth knowing: YouTube serves Opus at roughly 130-160 kbps, so neither setting
recovers detail that was never there. V0 is already well past transparent for
that source. Pick 320 only if a downstream tool or library expects it.

## How it works

```
React UI
  |  invoke("run_job", request)
  v
Rust command  (src-tauri/src/commands/media.rs)
  |
  +--> services/ytdlp.rs
  |      yt-dlp --dump-single-json        -> title, id, available caption languages
  |      yt-dlp -x --audio-format mp3 ... -> MP3 + .srt files, progress streamed back
  |
  +--> services/transcript.rs
  |      .srt -> timed cues -> timestamped .md   (free, instant)
  |      no captions? ffmpeg -> 16 kHz wav -> whisper-cli -osrt -> same path
  |
  v
JobResult { title, audioPath, transcript, warnings }
```

Progress events flow the other way on the `download://progress` channel, so the
meter updates while yt-dlp is still running.

### Files worth knowing

| Path | Responsibility |
|---|---|
| `src-tauri/src/commands/media.rs` | The only surface the UI can call. Validation + orchestration. |
| `src-tauri/src/services/ytdlp.rs` | Everything that talks to yt-dlp, including progress parsing. |
| `src-tauri/src/services/transcript.rs` | SRT parsing, markdown formatting, Whisper fallback. |
| `src-tauri/src/services/binaries.rs` | Finds the CLI tools. See the PATH note below. |
| `src/lib/api.ts` | Typed mirror of the Rust structs. Change one, change the other. |

---

## Three decisions worth remembering

**External binaries instead of bundled ones.** yt-dlp needs frequent updates to
keep working against YouTube. Bundling it as a Tauri sidecar would mean shipping
a new app build every time it breaks; shelling out to the Homebrew copy means
`brew upgrade yt-dlp` fixes it. The trade-off is that the app has install
prerequisites, so it checks for them on launch and says what's missing.

**Captions before Whisper.** Pulling existing captions is free and instant.
Whisper is minutes of CPU for the same result when captions exist. The app only
falls back when there is nothing to pull.

**Results are found by video id, not by predicted filename.** yt-dlp sanitises
titles in ways that are annoying to reproduce, so the output template embeds
`[video_id]` and Rust globs the folder for it afterwards. Deterministic without
having to mirror yt-dlp's escaping rules.

---

## The macOS PATH gotcha

An app launched from Finder does not inherit your shell `PATH`, so
`/opt/homebrew/bin` is missing and `Command::new("yt-dlp")` fails — but only in
the bundled build, never in `npm run dev`. `services/binaries.rs` probes the
known install directories first and falls back to `PATH`. If you install the
tools somewhere unusual, override with `YT_DLP_PATH` / `FFMPEG_PATH`.

---

## Next steps, in order of value

1. **A queue.** One `Vec<JobRequest>` processed sequentially, so you can paste
   several links and walk away. Roughly a day's work.
2. **Cancel.** Keep the `Child` handle in Tauri state and call `.kill()`.
3. **Timestamped transcript.** The `.srt` timings are already there — keep them
   in a second output file instead of throwing them away.
4. **Playlists.** Drop `--no-playlist`, loop over the entries from the JSON dump.

---

Note that downloading from YouTube goes against their terms of service. Sensible
uses are your own uploads, Creative Commons material, and personal transcription
of things you have the rights to.
