# Files on Convert wait for Start, and go to the library -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A file on Convert waits as a pill for Start and obeys the options, language included; everything a file makes goes into its own folder in the Save to folder, with an MP3 when Audio is ticked, and nothing is written beside the original.

**Architecture:** Backend: a new `services/mp3.rs` copies or encodes; `thumb::ensure_from` draws the picture from the original; `screen::read_screens` takes its cache path; `transcribe_file` makes a library folder with the same helper a download uses. Frontend: `PendingLink.file` lets a file sit among the links; Start maps it to a file job; the language Segmented moves from Settings into Convert's Transcript group; the Record dialog forces a transcript.

**Tech Stack:** Rust (tokio::process), Tauri v2, React 18 + TS, `node --test`.

**Spec:** `docs/superpowers/specs/2026-10-24-files-on-convert-design.md`

---

### Task 1: The pieces a file's folder is made of (Rust)

**Files:**
- Create: `src-tauri/src/services/mp3.rs`
- Modify: `src-tauri/src/services/mod.rs`, `src-tauri/src/services/thumb.rs`, `src-tauri/src/services/screen.rs`, `src-tauri/src/commands/media.rs` (one call)

- [ ] **Step 1: `mp3.rs` with tests first, stub bodies**

```rust
//! An MP3 of a file already on disk, for that file's folder in the library.
//!
//! The original is never touched. An MP3 is copied as it is -- encoding it
//! again would only lose quality -- and anything else, a video, a WAV, an
//! M4A, is encoded at the quality chosen for downloads.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use super::binaries;
use super::cancel;

/// Whether `source` has to be encoded, or can simply be copied.
pub fn needs_encoding(source: &Path) -> bool {
    let _ = source;
    false
}

/// LAME's settings for the UI's choice, matching what a download gets.
pub fn quality_args(choice: Option<&str>) -> [&'static str; 2] {
    let _ = choice;
    ["", ""]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_mp3_is_copied_as_it_is() {
        assert!(!needs_encoding(Path::new("/x/Recording 2026-10-24 16-20.mp3")));
        assert!(!needs_encoding(Path::new("/x/LOUD.MP3")));
    }

    #[test]
    fn anything_else_is_encoded() {
        for name in ["lecture.mp4", "talk.m4a", "raw.wav", "noext"] {
            assert!(needs_encoding(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn the_quality_matches_a_downloads() {
        assert_eq!(quality_args(Some("320")), ["-b:a", "320k"]);
        assert_eq!(quality_args(Some("v0")), ["-q:a", "0"]);
        assert_eq!(quality_args(None), ["-q:a", "0"]);
    }
}
```

`mod.rs`: add `pub mod mp3;` between `model` and `notes`.

Run `cd src-tauri && cargo test mp3::` -- expect 2 of 3 FAIL.

- [ ] **Step 2: Implement, then `keep`**

```rust
pub fn needs_encoding(source: &Path) -> bool {
    !source
        .extension()
        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("mp3"))
}

pub fn quality_args(choice: Option<&str>) -> [&'static str; 2] {
    match choice {
        Some("320") => ["-b:a", "320k"], // constant bitrate
        _ => ["-q:a", "0"],              // VBR V0, ~245 kbps average
    }
}

/// Put an MP3 of `source` at `destination`. Claimed first, so a Stop takes
/// back a half-written file.
pub async fn keep(source: &Path, destination: &Path, quality: Option<&str>) -> Result<PathBuf, String> {
    cancel::claim(destination);
    if !needs_encoding(source) {
        tokio::fs::copy(source, destination)
            .await
            .map_err(|e| format!("Could not copy the audio: {e}"))?;
        return Ok(destination.to_path_buf());
    }

    let ffmpeg = binaries::require("ffmpeg")?;
    let mut child = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args(["-vn", "-map", "0:a:0", "-c:a", "libmp3lame"])
        .args(quality_args(quality))
        .arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    let tracked = cancel::track(child.id());
    let status = child.wait().await.map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    drop(tracked);
    cancel::check()?;
    if !status.success() {
        return Err("ffmpeg could not make an MP3 of this file.".to_string());
    }
    Ok(destination.to_path_buf())
}
```

- [ ] **Step 3: `thumb::ensure_from`** -- after `ensure`:

```rust
/// The picture for a folder made from a file kept somewhere else: a frame of
/// that file if it is a video, its waveform if it is sound, and otherwise
/// whatever the folder itself offers. Nothing is written beside the file.
pub fn ensure_from(folder: &Path, source: &Path) -> Option<PathBuf> {
    if let Some(poster) = existing(folder) {
        return Some(poster);
    }
    let poster = folder.join(POSTER);
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let made = if VIDEO.contains(&ext.as_str()) {
        frame(source, &poster)
    } else if AUDIO.contains(&ext.as_str()) {
        waveform(source, &poster)
    } else {
        false
    };
    if made {
        Some(poster)
    } else {
        ensure(folder)
    }
}
```

- [ ] **Step 4: `screen.rs` takes its cache path**

- `fn cache_path` becomes `pub fn cache_path`, its comment's first line "What was read from a video, kept beside it." becomes "Where a download keeps what was read from its video: beside it."
- `fn cached(video: &Path) -> ...` becomes `fn cached(video: &Path, cache: &Path) -> ...`; delete its `let cache = cache_path(video);` and use `cache` (`std::fs::metadata(cache)`, `std::fs::read_to_string(cache)`).
- `read_screens(app: &AppHandle, video: &Path, work_dir: &Path)` becomes `read_screens(app: &AppHandle, video: &Path, cache: &Path, work_dir: &Path)`; `cached(video)` -> `cached(video, cache)`; `std::fs::write(cache_path(video), json)` -> `std::fs::write(cache, json)`.
- In `commands/media.rs` (the download's call), `screen::read_screens(app, &video, &work_dir)` -> `screen::read_screens(app, &video, &screen::cache_path(&video), &work_dir)`. The local call is rewritten in Task 2; until then change it to `screen::read_screens(&app, &path, &screen::cache_path(&path), &screens_work_dir())`.

- [ ] **Step 5:** `cd src-tauri && cargo test` -- 119 pass (116 + 3). Warnings for unused `keep`/`ensure_from` are expected until Task 2.

- [ ] **Step 6: Commit** -- "Copy or encode an MP3, and draw a picture, from a file kept elsewhere"

---

### Task 2: A file's job writes into its library folder (Rust)

**Files:** `src-tauri/src/commands/media.rs`, `src-tauri/src/services/transcript.rs`

- [ ] **Step 1: `LocalFileRequest`** -- after `read_screen`, add:

```rust
    /// Absolute folder path. Defaults to the library when empty.
    pub out_dir: Option<String>,
    /// An MP3 of the file in its library folder. Defaults to true.
    pub want_audio: Option<bool>,
    /// Defaults to true; Record always asks for one.
    pub want_transcript: Option<bool>,
    /// "v0" | "320", as for a download.
    pub audio_quality: Option<String>,
```

- [ ] **Step 2: split `job_folder`**

```rust
/// One folder per video, inside the library.
///
/// (existing doc comment kept)
fn job_folder(out_dir: &std::path::Path, title: &str, video_id: &str) -> Result<PathBuf, String> {
    library_folder(out_dir, &format!("{} [{video_id}]", combine::sanitise(title)))
}

/// A folder of its own in the library, for a download or a file on disk.
fn library_folder(out_dir: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    // 120 bytes keeps the whole path clear of the 255-byte filename limit.
    let trimmed: String = name.chars().take(120).collect();

    let folder = out_dir.join(trimmed.trim());
    // Claimed before it is made, so a stop knows whether it was ours to remove.
    cancel::claim(&folder);
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not create a folder in the library: {e}"))?;
    Ok(folder)
}
```

- [ ] **Step 3: rewrite `transcribe_file_inner`**

```rust
async fn transcribe_file_inner(app: &AppHandle, request: LocalFileRequest) -> Result<JobResult, String> {
    let app = app.clone();
    let path = validate_audio_path(&request.path)?;
    let out_dir = resolve_out_dir(request.out_dir.as_deref())?;
    let want_audio = request.want_audio.unwrap_or(true);
    let want_transcript = request.want_transcript.unwrap_or(true);
    if !want_audio && !want_transcript {
        return Err("Choose audio or a transcript to keep.".to_string());
    }

    let model_path = if want_transcript {
        let model = request
            .whisper_model_path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .ok_or_else(|| "Choose a Whisper model under Options first.".to_string())?;
        if binaries::resolve("whisper-cli").is_none() && binaries::resolve("whisper-cpp").is_none() {
            return Err(
                "whisper-cli is not installed. Install it with: brew install whisper-cpp".to_string(),
            );
        }
        Some(PathBuf::from(model))
    } else {
        None
    };

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string());

    // The original stays exactly where it is, untouched. Everything this job
    // makes goes into a folder of its own in the library, as a download's does.
    let name = combine::sanitise(&title);
    let job_dir = library_folder(&out_dir, &name)?;
    let named = |extension: &str| job_dir.join(format!("{name}.{extension}"));

    let audio_path = if want_audio {
        ytdlp::emit(&app, ProgressEvent::stage("converting", Some(title.clone())));
        Some(mp3::keep(&path, &named("mp3"), request.audio_quality.as_deref()).await?)
    } else {
        None
    };
    cancel::check()?;

    let transcript = match model_path {
        Some(model_path) => {
            let language = request
                .language
                .as_deref()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .unwrap_or("auto")
                .to_string();
            let read_screen = request.read_screen.unwrap_or(false);

            ytdlp::emit(&app, ProgressEvent::stage("transcribing", Some(title.clone())));
            let screens = if read_screen && has_picture(&path) {
                screen::read_screens(&app, &path, &named("screens.json"), &screens_work_dir())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            cancel::check()?;

            ytdlp::emit(&app, ProgressEvent::stage("transcribing", Some(title.clone())));
            cancel::claim(&named("md"));
            cancel::claim(&named("pdf"));

            let srt = transcript::whisper_srt(&app, &path, &model_path, &language).await?;
            let cues = transcript::parse_srt(&srt);
            if cues.is_empty() {
                return Err("The transcript came back empty.".to_string());
            }

            let files = write_transcript_outputs(
                &named("md"),
                &named("pdf"),
                request.transcript_format.as_deref(),
                &TranscriptHeader {
                    title: &title,
                    url: &path.to_string_lossy(),
                    uploader: None,
                    duration_seconds: None,
                    source: "whisper",
                    language: Some(&language),
                    read_screen,
                },
                &cues,
                &screens,
                &[],
            )?;

            Some(Transcript {
                text: transcript::cues_to_plain_text(&cues),
                source: "whisper".to_string(),
                language: Some(language),
                file_path: files.markdown_path,
                pdf_path: files.pdf_path,
                markdown: Some(files.markdown),
            })
        }
        None => None,
    };

    thumb::ensure_from(&job_dir, &path);

    ytdlp::emit(&app, ProgressEvent::stage("done", None));

    Ok(JobResult {
        title,
        folder: job_dir.to_string_lossy().into_owned(),
        audio_path: audio_path.map(|p| p.to_string_lossy().into_owned()),
        video_path: None,
        transcript,
        warnings: Vec::new(),
    })
}
```

Add `use crate::services::mp3;` beside the other service imports.

- [ ] **Step 4: delete `transcript_path_for`** in `services/transcript.rs` (the function and its doc comment) and its two tests, `transcript_path_uses_the_plain_name_when_free` and `transcript_path_steps_aside_rather_than_overwrite`. Nothing else calls it: the library folder is the app's own, so a file there is the app's to replace.

- [ ] **Step 5:** `cd src-tauri && cargo test` -- 117 pass (119 - 2), no warnings.

- [ ] **Step 6: Commit** -- "Write what a file makes into its own folder in the library"

---

### Task 3: The request carries the options (TS)

**Files:** `src/lib/api.ts`, `src/lib/queue.ts`, `src/lib/useJobs.ts`, new `src/lib/queue.test.ts`

- [ ] **Step 1: test** `src/lib/queue.test.ts`:

```ts
// Run: node --test src/lib/queue.test.ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { fileItem, pendingFile } from "./queue.ts";

test("a file waits as a pill named after it, with nothing to read", () => {
  const pill = pendingFile("/Users/x/Documents/Snimci/Lecture 3.mp4");
  assert.equal(pill.url, "/Users/x/Documents/Snimci/Lecture 3.mp4");
  assert.equal(pill.title, "Lecture 3.mp4");
  assert.equal(pill.file, true);
  assert.equal(pill.failed, false);
});

test("a recording from Record always asks for a transcript", () => {
  assert.equal(fileItem("/x/a.mp3", "sr", true).wantTranscript, true);
  assert.equal(fileItem("/x/a.mp3").wantTranscript, undefined);
});
```

Run it: FAIL (`pendingFile` missing).

- [ ] **Step 2: `queue.ts`**

In `QueueItem`, after `language?: string | null;`:

```ts
  /** Set by Record, whose button says Transcribe; otherwise the options decide. */
  wantTranscript?: boolean;
```

Replace `fileItem` with:

```ts
export function fileItem(
  path: string,
  language: string | null = null,
  wantTranscript?: boolean,
): QueueItem {
  return {
    id: crypto.randomUUID(),
    kind: "file",
    label: path.split("/").pop() ?? path,
    target: path,
    status: "waiting",
    language,
    wantTranscript,
  };
}
```

In `PendingLink`, after `playlist?: boolean;`:

```ts
  /** A file on disk waiting among the links; `url` is its path. */
  file?: boolean;
```

After `pendingLink`:

```ts
/** A file on disk, waiting on Convert for Start like a link. */
export function pendingFile(path: string): PendingLink {
  return {
    id: crypto.randomUUID(),
    url: path,
    title: path.split("/").pop() ?? path,
    failed: false,
    file: true,
  };
}
```

Run the test: PASS.

- [ ] **Step 3: `api.ts`** -- in `LocalFileRequest`, after `readScreen: boolean;`:

```ts
  /** Absolute folder path; null means the library. */
  outDir: string | null;
  wantAudio: boolean;
  wantTranscript: boolean;
  audioQuality: AudioQuality;
```

`AudioQuality` is already defined in `api.ts` for `JobRequest`; if it lives only in `settings.ts`, use `string` instead.

- [ ] **Step 4: `useJobs.ts`** -- the file branch of `runOne` becomes:

```ts
    if (item.kind === "file") {
      return transcribeFile({
        path: item.target,
        whisperModelPath: s.whisperModelPath || null,
        language: item.language ?? s.transcriptLangs[0] ?? null,
        transcriptFormat: s.transcriptFormat,
        readScreen: s.readScreen,
        outDir: s.outDir || null,
        wantAudio: s.wantAudio,
        wantTranscript: item.wantTranscript ?? s.wantTranscript,
        audioQuality: s.audioQuality,
      });
    }
```

- [ ] **Step 5:** `npm run build:vite` clean; commit -- "Send a file's options with it"

---

### Task 4: Files wait on Convert; language moves there (TS)

**Files:** `src/App.tsx`, `src/components/ConvertView.tsx`, `src/components/LinkField.tsx`, `src/components/SettingsView.tsx`, `src/styles.css`

- [ ] **Step 1: `App.tsx`**
  - Import `pendingFile` from `./lib/queue`.
  - Replace the comment above `transcribe` with `// A recording from Record is asked which language it is spoken in first.`
  - After `transcribe`, add:

```ts
  /** Files on disk wait on Convert as pills beside the links, for Start. */
  function addFiles(paths: string[]) {
    setLinks((prev) => {
      const known = new Set(prev.map((l) => l.url));
      return [...prev, ...paths.filter((path) => !known.has(path)).map(pendingFile)];
    });
  }
```

  - Drag-drop effect: comment -> `// Files dropped anywhere on the window wait on Convert, beside the links.`; `transcribe(event.payload.paths);` -> `addFiles(event.payload.paths);`.
  - `<ConvertView ... onTranscribe={transcribe}` -> `onAddFiles={addFiles}`.
  - The dialog's `onConfirm`: `fileItem(file.path, file.language)` -> `fileItem(file.path, file.language, true)`.

- [ ] **Step 2: `ConvertView.tsx`**
  - Prop `onTranscribe: (paths: string[]) => void;` and its comment -> `/** Files chosen here wait as pills, like links, until Start. */ onAddFiles: (paths: string[]) => void;`; destructure `onAddFiles` instead of `onTranscribe`; in `chooseFiles`, `onTranscribe(paths);` -> `onAddFiles(paths);`.
  - Import `fileItem` again: `import { fileItem, urlItem, type Pasted, type PendingLink } from "../lib/queue";`; import `Segmented` with `Check`: `import { Check, Segmented } from "./Controls";`.
  - `start()`:

```ts
  function start() {
    if (links.length === 0) return;
    const items = links.map((l) =>
      l.file ? fileItem(l.url) : urlItem(l.url, l.title ?? l.url, l.title !== null),
    );
    onClearLinks();
    void jobs.enqueue(items);
  }
```

  - Footer: `` footer = `${links.length} ${links.length === 1 ? "link" : "links"} ready`; `` -> `` footer = `${links.length} ready`; ``
  - In the Transcript group, after its `</div>` closing `.checks` and before the group's closing `</div>`:

```tsx
          <div className="opts__language">
            <Segmented<string>
              label="Language spoken"
              value={settings.transcriptLangs[0] === "en" ? "en" : "sr"}
              onChange={(lang) =>
                update({ transcriptLangs: lang === "sr" ? ["sr", "en"] : ["en", "sr"] })
              }
              options={[
                { value: "sr", label: "Serbian" },
                { value: "en", label: "English" },
              ]}
            />
          </div>
```

- [ ] **Step 3: `LinkField.tsx`** -- the remove button's `aria-label="Remove link"` -> `aria-label={link.file ? "Remove file" : "Remove link"}`.

- [ ] **Step 4: `SettingsView.tsx`** -- delete the whole `<Setting title="Caption language to try first" ...>...</Setting>` block. `Segmented` stays imported (Surface and Aurora use it).

- [ ] **Step 5: `styles.css`** -- after the `.checks` rule:

```css
/* The language spoken, under the transcript's ticks. */
.opts__language {
  margin-top: 0.75rem;
}
```

- [ ] **Step 6:** `npm run build:vite` clean; `grep -n "jobs.enqueue" src` shows only `App.tsx` (the dialog) and `ConvertView.tsx` (Start). Commit -- "Let a file wait on Convert for Start, and choose its language there"

---

### Task 5: Write it down

- [ ] `CHANGELOG.md`, Unreleased:
  - Improved: "A file dropped on Convert waits for Start like a link, so you can set its options first, the language spoken among them."
  - Fixed: "A file or recording you transcribe now goes into the folder you save to, in a folder of its own with an MP3, instead of beside the original."
- [ ] `CLAUDE.md`: services list, after `` `language` hearing which language a file is in `` add `` · `mp3` an MP3 of a file on disk``.
- [ ] `python3 scripts/changelog.py text Unreleased` parses. Commit -- "Write down where a file's transcript goes"
- [ ] Memory `local-audio-transcription.md`: the "beside the original, never overwrite" decision is replaced by "its own library folder, original untouched" (outside the repo, no commit).

---

### Task 6: Prove it

- [ ] test-runner: `cargo test` (117), `build:vite`, `test_changelog.py`, `node --test src/lib/*.test.ts`.
- [ ] reviewer on the diff since the plan's commit.
- [ ] Ask the owner, then `npm run install:app`.
- [ ] Owner: drop a video on Convert -> a pill, nothing starts; tick Read the screen, Start -> `Tzz Library/<name>/` holds `.md`, `.mp3`, `poster.jpg`, `.screens.json`; nothing new beside the original.
- [ ] Owner: Transcribe a recording from Record -> the dialog, then a folder in the library.
