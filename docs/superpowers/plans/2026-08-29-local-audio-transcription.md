# Local Audio Transcription Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user drop a local audio file on the window (or pick it from Finder) and get a timestamped markdown transcript saved beside it.

**Architecture:** A second entry point into the transcription pipeline that already exists. `transcript::whisper_srt()` accepts an arbitrary audio path and converts it through ffmpeg itself, so no transcription logic is written -- only a new thin command that skips yt-dlp, plus the UI to reach it. The existing `JobResult` shape is reused so the result panel needs no changes.

**Tech Stack:** Rust / Tauri v2, React 18 + TypeScript, whisper.cpp (`whisper-cli`), ffmpeg.

**Spec:** `docs/superpowers/specs/2026-08-29-local-audio-transcription-design.md`

> **No git in this project.** `.git` does not exist, so the usual per-task commit step is replaced by a verification step. If `git init` is run later, commit at each of those points instead.

> **UI language:** the existing interface is English ("YouTube link", "Options", "Download"). New UI strings stay English for consistency.

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src-tauri/src/services/transcript.rs` | Modify | Add `transcript_path_for` + `write_markdown`; `save_markdown` delegates to the latter |
| `src-tauri/src/commands/media.rs` | Modify | Add `LocalFileRequest`, `supported_extension`, `transcribe_file` command |
| `src-tauri/src/lib.rs` | Modify:8-13 | Register the new command |
| `src/lib/api.ts` | Modify | Mirror the Rust struct, add `transcribeFile` |
| `src/App.tsx` | Modify | Dropzone, file picker, drag-drop listener, `startLocal` |
| `src/styles.css` | Modify | `.dropzone` styles |

---

### Task 1: Non-destructive transcript path

`save_markdown` writes `<stem>.md`, overwriting whatever is there. Harmless for a
just-downloaded MP3, destructive for a file the user already owned.

**Files:**
- Modify: `src-tauri/src/services/transcript.rs:298-304` (`save_markdown`)
- Test: `src-tauri/src/services/transcript.rs` (the existing `mod tests`)

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    #[test]
    fn transcript_path_uses_the_plain_name_when_free() {
        let dir = std::env::temp_dir().join("yt-mp3-test-free");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("interview.mp3");

        assert_eq!(transcript_path_for(&audio), dir.join("interview.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn transcript_path_steps_aside_rather_than_overwrite() {
        let dir = std::env::temp_dir().join("yt-mp3-test-taken");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("interview.mp3");
        std::fs::write(dir.join("interview.md"), "my own notes").unwrap();

        assert_eq!(transcript_path_for(&audio), dir.join("interview.transcript.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run the tests and watch them fail**

```bash
cd src-tauri && cargo test transcript_path
```

Expected: FAIL, `cannot find function transcript_path_for in this scope`.

- [ ] **Step 3: Implement**

Replace `save_markdown` with:

```rust
/// Where a local file's transcript should go.
///
/// A downloaded MP3 owns its `.md` neighbour, but a file the user already had
/// may not: `interview.mp3` must not clobber an `interview.md` they wrote
/// themselves. When the plain name is taken we step aside.
pub fn transcript_path_for(audio_path: &Path) -> PathBuf {
    let plain = audio_path.with_extension("md");
    if !plain.exists() {
        return plain;
    }
    let stem = audio_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".to_string());
    audio_path.with_file_name(format!("{stem}.transcript.md"))
}

/// Write the transcript next to the MP3 so it survives the app closing.
pub fn save_markdown(audio_path: &Path, markdown: &str) -> Result<PathBuf, String> {
    write_markdown(&audio_path.with_extension("md"), markdown)
}

pub fn write_markdown(md_path: &Path, markdown: &str) -> Result<PathBuf, String> {
    std::fs::write(md_path, markdown)
        .map_err(|e| format!("Could not save the transcript: {e}"))?;
    Ok(md_path.to_path_buf())
}
```

Note `save_markdown` keeps its old behaviour on purpose -- the YouTube flow is untouched.

- [ ] **Step 4: Run the tests and watch them pass**

```bash
cd src-tauri && cargo test
```

Expected: PASS, including the three pre-existing SRT parser tests.

---

### Task 2: Extension validation

**Files:**
- Modify: `src-tauri/src/commands/media.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `media.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_audio_and_video_containers() {
        for name in ["a.mp3", "a.M4A", "a.wav", "a.mp4", "a.webm"] {
            assert!(supported_extension(std::path::Path::new(name)), "{name}");
        }
    }

    #[test]
    fn rejects_everything_else() {
        for name in ["notes.txt", "archive.zip", "noextension"] {
            assert!(!supported_extension(std::path::Path::new(name)), "{name}");
        }
    }
}
```

- [ ] **Step 2: Run and watch it fail**

```bash
cd src-tauri && cargo test supported_extension
```

Expected: FAIL, `cannot find function supported_extension`.

- [ ] **Step 3: Implement**

Add near the top of `media.rs`, after the imports:

```rust
/// Containers ffmpeg handles in practice; whisper never sees them directly.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm",
];

fn supported_extension(path: &std::path::Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext.as_str()))
}
```

- [ ] **Step 4: Run and watch it pass**

```bash
cd src-tauri && cargo test
```

Expected: PASS.

---

### Task 3: The `transcribe_file` command

**Files:**
- Modify: `src-tauri/src/commands/media.rs`
- Modify: `src-tauri/src/lib.rs:8-13`

- [ ] **Step 1: Add the request struct**

Below `JobRequest`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileRequest {
    /// Absolute path to an audio or video file already on disk.
    pub path: String,
    pub whisper_model_path: Option<String>,
    /// Whisper language hint. `None` means let it detect.
    pub language: Option<String>,
}
```

- [ ] **Step 2: Add the command**

```rust
/// Transcribe a file the user already has. No download, no MP3 conversion --
/// `whisper_srt` feeds it through ffmpeg itself.
#[tauri::command]
pub async fn transcribe_file(app: AppHandle, request: LocalFileRequest) -> Result<JobResult, String> {
    let path = validate_audio_path(&request.path)?;

    let model_path = request
        .whisper_model_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "Choose a Whisper model under Options first.".to_string())?;

    if binaries::resolve("whisper-cli").is_none() && binaries::resolve("whisper-cpp").is_none() {
        return Err("whisper-cli is not installed. Install it with: brew install whisper-cpp".to_string());
    }

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string());
    let language = request
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or("auto")
        .to_string();

    ytdlp::emit(&app, ProgressEvent::stage("transcribing", Some(title.clone())));

    let srt = transcript::whisper_srt(&path, &PathBuf::from(model_path), &language).await?;
    let cues = transcript::parse_srt(&srt);
    if cues.is_empty() {
        return Err("The transcript came back empty.".to_string());
    }

    let markdown = transcript::cues_to_markdown(
        &TranscriptHeader {
            title: &title,
            url: &path.to_string_lossy(),
            uploader: None,
            duration_seconds: None,
            source: "whisper",
            language: Some(&language),
        },
        &cues,
    );
    let saved = transcript::write_markdown(&transcript::transcript_path_for(&path), &markdown)?;

    ytdlp::emit(&app, ProgressEvent::stage("done", None));

    Ok(JobResult {
        title,
        audio_path: path.to_string_lossy().into_owned(),
        transcript: Some(Transcript {
            text: transcript::cues_to_plain_text(&cues),
            source: "whisper".to_string(),
            language: Some(language),
            file_path: Some(saved.to_string_lossy().into_owned()),
        }),
        warnings: Vec::new(),
    })
}

fn validate_audio_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Choose an audio file first.".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_file() {
        return Err("That file is no longer there.".to_string());
    }
    if !supported_extension(&path) {
        return Err(
            "That file type is not supported. Try MP3, M4A, WAV, FLAC, OGG, OPUS, AAC, MP4, MOV, MKV or WEBM."
                .to_string(),
        );
    }
    Ok(path)
}
```

- [ ] **Step 3: Register it**

In `src-tauri/src/lib.rs`, add to `generate_handler!`:

```rust
            commands::media::transcribe_file,
```

- [ ] **Step 4: Verify it compiles and tests still pass**

```bash
cd src-tauri && cargo test
```

Expected: PASS, no warnings about unused imports.

---

### Task 4: Frontend API binding

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add the type and the call**

After `JobRequest`:

```ts
export type LocalFileRequest = {
  path: string;
  whisperModelPath: string | null;
  /** null lets Whisper detect the language. */
  language: string | null;
};
```

After `runJob`:

```ts
export function transcribeFile(request: LocalFileRequest): Promise<JobResult> {
  return invoke("transcribe_file", { request });
}
```

- [ ] **Step 2: Typecheck**

```bash
npm run build:vite
```

Expected: PASS.

---

### Task 5: Dropzone UI

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Imports and state**

Add to the `./lib/api` import list: `transcribeFile`. Add a new import:

```ts
import { getCurrentWebview } from "@tauri-apps/api/webview";
```

Add beside the other `useState` calls:

```ts
const [dragging, setDragging] = useState(false);
```

- [ ] **Step 2: The local job runner**

Add next to `start()`:

```tsx
  async function startLocal(path: string) {
    if (running) return;

    setError(null);
    setResult(null);
    setCopied(false);
    setStage("transcribing");
    setPercent(null);
    setDetail(null);

    try {
      const job = await transcribeFile({
        path,
        whisperModelPath: settings.whisperModelPath || null,
        language: null,
      });
      setResult(job);
      setStage("done");
      setPercent(100);
    } catch (caught) {
      setError(errorMessage(caught));
      setStage("idle");
      setPercent(null);
    }
  }

  async function pickAudioFile() {
    const picked = await open({
      multiple: false,
      filters: [
        {
          name: "Audio",
          extensions: ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm"],
        },
      ],
    });
    if (typeof picked === "string") {
      await startLocal(picked);
    }
  }
```

- [ ] **Step 3: The drag-drop listener**

Add after the `onProgress` effect. It re-subscribes when `running` or the model
path changes so the handler never closes over stale state:

```tsx
  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragging(true);
      } else if (event.payload.type === "leave") {
        setDragging(false);
      } else if (event.payload.type === "drop") {
        setDragging(false);
        const first = event.payload.paths[0];
        if (first) void startLocal(first);
      }
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, [running, settings.whisperModelPath]);
```

- [ ] **Step 4: The markup**

Inside `<section className="deck">`, after `{detail && <p className="detail">{detail}</p>}`:

```tsx
        <div className={`dropzone ${dragging ? "dropzone--active" : ""}`}>
          <span>Or drop an audio file here to transcribe it</span>
          <button
            type="button"
            className="ghost-button"
            onClick={pickAudioFile}
            disabled={running}
          >
            Choose file
          </button>
        </div>
```

- [ ] **Step 5: The styles**

Append to `src/styles.css`:

```css
/* --- Dropzone: local files bypass YouTube entirely --------------------- */

.dropzone {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px dashed var(--line);
  border-radius: var(--radius);
  color: var(--muted);
  font-family: var(--mono);
  font-size: 12px;
  transition: border-color 120ms ease, background 120ms ease;
}

.dropzone--active {
  border-color: var(--signal);
  background: var(--panel-raised);
  color: var(--text);
}
```

- [ ] **Step 6: Typecheck**

```bash
npm run build:vite
```

Expected: PASS.

---

### Task 6: End-to-end verification

- [ ] **Step 1: Run the app**

```bash
npm run dev
```

- [ ] **Step 2: Set the model**

Options -> Whisper model -> Choose ->
`~/Library/Application Support/yt-mp3/models/ggml-large-v3-turbo.bin`

- [ ] **Step 3: Drop a local MP3 on the window**

Expected: the meter switches to "Transcribing", then "Finished"; the result panel
shows the file name, the transcript text, and a "Show file" button.

- [ ] **Step 4: Check the output on disk**

```bash
ls -l <folder holding the test mp3>
```

Expected: `<name>.md` beside the audio, containing a header and `**[mm:ss]**`
paragraphs.

- [ ] **Step 5: Check the guard clauses**

Clear the model path in Options, drop the file again.
Expected: "Choose a Whisper model under Options first." and no crash.

Drop a `.txt` file.
Expected: the unsupported-file sentence.

- [ ] **Step 6: Check the no-overwrite rule**

With `<name>.md` already present from step 3, drop the same MP3 again.
Expected: a new `<name>.transcript.md`, the original `<name>.md` untouched.
