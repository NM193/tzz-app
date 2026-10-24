# Asking which language is spoken -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Before a recording or a local file is transcribed, a dialog shows the language Whisper hears, already chosen, and lets the owner correct it; the choice reaches `whisper-cli -l`.

**Architecture:** A new Rust service, `services/language.rs`, cuts 30 s out of the file with ffmpeg and asks `whisper-cli -l auto -dl` what it hears; a thin `detect_language` command exposes it. On the frontend, every path that used to go straight into `jobs.enqueue` goes to a `LanguagePrompt` dialog owned by `App.tsx`, which detects one file at a time and queues each with its language. `QueueItem` carries the language to `transcribe_file`, which already accepts one.

**Tech Stack:** Rust (tokio::process), Tauri v2 commands, React 18 + TypeScript, `node --test` for pure TS logic.

**Spec:** `docs/superpowers/specs/2026-10-24-transcription-language-design.md`

**Measured facts this plan relies on (2026-10-24, bundled whisper-cli):**
- `whisper-cli -l auto -dl` logs `whisper_full_with_state: auto-detected language: hr (p = 0.526195)` on **stderr only**, and `-np` suppresses it entirely. Do not pass `-np`.
- `ffmpeg -hide_banner -i <file>` with no output prints `Duration: 00:00:12.07, ...` on stderr and exits 1, always.

---

## Files

| File | What it does |
|---|---|
| `src-tauri/src/services/language.rs` (new) | Duration, excerpt, Whisper's guess; three pure functions with tests |
| `src-tauri/src/services/mod.rs` | `pub mod language;` |
| `src-tauri/src/commands/media.rs` | `detect_language` command, validation only |
| `src-tauri/src/lib.rs` | registers the command |
| `src/lib/language.ts` (new) | what the dialog offers and what each choice sends -- pure |
| `src/lib/language.test.ts` (new) | `node --test` for the above |
| `src/lib/api.ts` | `DetectedLanguage`, `detectLanguage` |
| `src/lib/queue.ts` | `QueueItem.language`, `fileItem(path, language)` |
| `src/lib/useJobs.ts` | `runOne` sends `item.language` |
| `src/lib/motion.ts` | `sheetEnter`, the dialog's wipe from below |
| `src/components/LanguagePrompt.tsx` (new) | the dialog |
| `src/App.tsx` | owns `asking`, `transcribe(paths)`, renders the dialog |
| `src/components/ConvertView.tsx` | `onTranscribe` replaces its own enqueue |
| `src/components/RecordView.tsx` | `onTranscribe` replaces the `jobs` prop |
| `src/styles.css` | the dialog |
| `CHANGELOG.md`, `CLAUDE.md` | one New line; the service and the feature listed |

---

### Task 1: Hear the language in Rust

**Files:**
- Create: `src-tauri/src/services/language.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `docs/superpowers/specs/2026-10-24-transcription-language-design.md` (one sentence)

- [ ] **Step 1: Write the module with its tests and stub bodies**

`src-tauri/src/services/language.rs`:

```rust
//! Which language a file is spoken in, asked of Whisper before transcribing.
//!
//! Whisper's guess is only ever a default. It decides from the first 30
//! seconds it is given, and on 2026-10-24 Serbian over music came back as
//! English, so the dialog shows the guess and the owner confirms or corrects
//! it. This module hears; naming and grouping languages is the dialog's job.
//!
//! Deliberately not registered with `cancel`: Stop on a running job must not
//! reach a detection, nor a detection be mistaken for the job.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tokio::process::Command;

use super::binaries;

/// All Whisper listens to when it guesses, so all there is to cut.
const EXCERPT_SECONDS: f64 = 30.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    /// Whisper's code: "sr", "hr", "en", ...
    pub code: String,
    /// 0.0-1.0, how sure it was.
    pub probability: f32,
}

/// `Duration: 00:12:34.56` from ffmpeg's description of an input.
pub fn parse_duration(header: &str) -> Option<f64> {
    let _ = header;
    None
}

/// Where the excerpt starts.
pub fn excerpt_start(duration: Option<f64>) -> f64 {
    let _ = duration;
    -1.0
}

/// `auto-detected language: hr (p = 0.526195)` from whisper-cli's log.
pub fn parse_detected(line: &str) -> Option<Detected> {
    let _ = line;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_duration_ffmpeg_prints_for_an_input() {
        let header = "Input #0, mp3, from 'x.mp3':\n  Duration: 00:00:12.07, start: 0.023021, bitrate: 125 kb/s\n";
        assert_eq!(parse_duration(header), Some(12.07));
        assert_eq!(parse_duration("  Duration: 01:04:30.50, start: 0.000000"), Some(3870.5));
    }

    #[test]
    fn a_stream_of_unknown_length_has_no_duration() {
        assert_eq!(parse_duration("  Duration: N/A, start: 0.000000, bitrate: N/A"), None);
        assert_eq!(parse_duration("At least one output file must be specified"), None);
    }

    #[test]
    fn listens_a_third_of_the_way_in_so_an_intro_does_not_decide() {
        assert_eq!(excerpt_start(Some(3600.0)), 1200.0);
        assert_eq!(excerpt_start(Some(90.0)), 30.0);
    }

    #[test]
    fn never_starts_so_late_that_less_than_thirty_seconds_are_left() {
        // A third of 40 s is 13.3 s, which would leave 26.7 s.
        assert_eq!(excerpt_start(Some(40.0)), 10.0);
    }

    #[test]
    fn a_short_or_unmeasured_file_is_heard_from_the_start() {
        assert_eq!(excerpt_start(Some(30.0)), 0.0);
        assert_eq!(excerpt_start(Some(12.07)), 0.0);
        assert_eq!(excerpt_start(None), 0.0);
    }

    #[test]
    fn reads_whispers_guess_from_its_log_line() {
        let heard = parse_detected("whisper_full_with_state: auto-detected language: hr (p = 0.526195)")
            .expect("a guess");
        assert_eq!(heard.code, "hr");
        assert!((heard.probability - 0.526195).abs() < 1e-6);
    }

    #[test]
    fn ignores_whispers_other_chatter() {
        for line in [
            "whisper_init_from_file_with_params_no_state: loading model",
            "",
            "auto-detected language:  (p = 0.5)",
        ] {
            assert_eq!(parse_detected(line), None, "{line}");
        }
    }
}
```

Add `pub mod language;` to `src-tauri/src/services/mod.rs`, between `glass` and `library` (the list is alphabetical).

- [ ] **Step 2: Run the tests, see them fail**

Run: `cd src-tauri && cargo test language::`
Expected: 5 of 7 FAIL (the two "no duration"/"chatter" tests pass against stubs returning `None`).

- [ ] **Step 3: Implement the three functions**

Replace the three stubs:

```rust
/// `Duration: 00:12:34.56` from ffmpeg's description of an input. `None` for
/// `Duration: N/A` -- a stream that does not know its length -- or no line.
pub fn parse_duration(header: &str) -> Option<f64> {
    let rest = header.split("Duration: ").nth(1)?;
    let clock = rest.split(',').next()?.trim();
    let mut parts = clock.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Where the excerpt starts: a third of the way in, so a lecture's intro
/// music does not decide, but never so late that less than 30 s is left.
/// A short file, or one whose length is unknown, is heard from the start.
pub fn excerpt_start(duration: Option<f64>) -> f64 {
    match duration {
        Some(total) if total > EXCERPT_SECONDS => (total / 3.0).min(total - EXCERPT_SECONDS),
        _ => 0.0,
    }
}

/// `auto-detected language: hr (p = 0.526195)` from whisper-cli's log.
pub fn parse_detected(line: &str) -> Option<Detected> {
    let rest = line.split("auto-detected language: ").nth(1)?;
    let (code, rest) = rest.split_once(" (p = ")?;
    let code = code.trim();
    if code.is_empty() {
        return None;
    }
    let probability = rest.trim_end().strip_suffix(')')?.parse().ok()?;
    Some(Detected { code: code.to_string(), probability })
}
```

- [ ] **Step 4: Run the tests, see them pass**

Run: `cd src-tauri && cargo test language::`
Expected: 7 passed.

- [ ] **Step 5: Add `detect`, the part that runs the tools**

Below `parse_detected`, above the tests:

```rust
/// Ask Whisper which language `audio_path` is spoken in.
pub async fn detect(audio_path: &Path, model_path: &Path) -> Result<Detected, String> {
    if !model_path.is_file() {
        return Err("Choose a Whisper model under Options first.".to_string());
    }
    let ffmpeg = binaries::require("ffmpeg")?;
    let whisper = binaries::resolve("whisper-cli")
        .or_else(|| binaries::resolve("whisper-cpp"))
        .ok_or_else(|| "whisper-cli is not installed.".to_string())?;

    // An input and no output: ffmpeg describes the file and exits 1, always.
    let probe = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(audio_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    let start = excerpt_start(parse_duration(&String::from_utf8_lossy(&probe.stderr)));

    let excerpt = excerpt_path();
    let heard = listen(&ffmpeg, &whisper, audio_path, model_path, &excerpt, start).await;
    let _ = std::fs::remove_file(&excerpt);
    heard
}

async fn listen(
    ffmpeg: &Path,
    whisper: &Path,
    audio_path: &Path,
    model_path: &Path,
    excerpt: &Path,
    start: f64,
) -> Result<Detected, String> {
    // Whisper only reads 16 kHz mono PCM.
    let cut = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{start:.3}"))
        .arg("-t")
        .arg(format!("{EXCERPT_SECONDS}"))
        .arg("-i")
        .arg(audio_path)
        .args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(excerpt)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    if !cut.success() {
        return Err("ffmpeg could not read the file.".to_string());
    }

    // whisper.cpp logs its guess on stderr, and `-np` would silence it.
    let heard = Command::new(whisper)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(excerpt)
        .args(["-l", "auto", "-dl"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Could not run whisper: {e}"))?;
    String::from_utf8_lossy(&heard.stderr)
        .lines()
        .find_map(parse_detected)
        .ok_or_else(|| "Whisper could not tell which language this is.".to_string())
}

/// A name of its own for each excerpt: two detections may overlap, one for a
/// dialog and one left over from a dialog that has closed.
fn excerpt_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("yt-mp3-language-{}-{n}.wav", std::process::id()))
}
```

- [ ] **Step 6: Build and test**

Run: `cd src-tauri && cargo test`
Expected: all pass (109 before + 7 new = 116). A `dead_code` warning for `detect` is expected until Task 2.

- [ ] **Step 7: Correct the spec**

In `docs/superpowers/specs/2026-10-24-transcription-language-design.md`, step 1 under "How it works" ends "That call always exits non-zero; only a missing `Duration:` line is a failure." Replace that sentence with: "That call always exits non-zero. A file without a `Duration:` line, or with `Duration: N/A`, is heard from the start."

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/services/language.rs src-tauri/src/services/mod.rs docs/superpowers/specs/2026-10-24-transcription-language-design.md
git commit -m "Ask Whisper which language a file is spoken in"
```

---

### Task 2: The `detect_language` command

**Files:**
- Modify: `src-tauri/src/commands/media.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the command**

In `src-tauri/src/commands/media.rs`, add to the `use crate::services::...` block:

```rust
use crate::services::language::{self, Detected};
```

After `LocalFileRequest`, add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectRequest {
    /// Absolute path to an audio or video file already on disk.
    pub path: String,
    pub whisper_model_path: Option<String>,
}
```

After `transcribe_file_inner`, add:

```rust
/// Which language a file is spoken in, for the dialog to suggest. Not a job:
/// nothing is written that a Stop would need to take back.
#[tauri::command]
pub async fn detect_language(request: DetectRequest) -> Result<Detected, String> {
    let path = validate_audio_path(&request.path)?;
    let model_path = request
        .whisper_model_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "Choose a Whisper model under Options first.".to_string())?;
    language::detect(&path, &PathBuf::from(model_path)).await
}
```

- [ ] **Step 2: Register it**

In `src-tauri/src/lib.rs`, in `generate_handler![...]`, after `commands::media::transcribe_file,` add:

```rust
            commands::media::detect_language,
```

- [ ] **Step 3: Build and test**

Run: `cd src-tauri && cargo test`
Expected: 116 passed, no warnings from `language.rs`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/media.rs src-tauri/src/lib.rs
git commit -m "Let the window ask which language a file is in"
```

---

### Task 3: What the dialog offers

**Files:**
- Create: `src/lib/language.ts`
- Create: `src/lib/language.test.ts`

- [ ] **Step 1: Write the test**

`src/lib/language.test.ts`:

```ts
// Run: node --test src/lib/language.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import { choiceFor, choicesFor, detectedLabel, languageName } from "./language.ts";

test("Serbian, Croatian and Bosnian are all Serbian, which sends sr", () => {
  for (const code of ["sr", "hr", "bs"]) assert.equal(choiceFor(code), "sr");
  assert.equal(choiceFor("en"), "en");
  assert.equal(choiceFor("de"), "de");
});

test("Serbian and English are always offered; anything else gets its own button", () => {
  assert.deepEqual(choicesFor(null), ["sr", "en"]);
  assert.deepEqual(choicesFor("hr"), ["sr", "en"]);
  assert.deepEqual(choicesFor("en"), ["sr", "en"]);
  assert.deepEqual(choicesFor("de"), ["sr", "en", "de"]);
});

test("languages are named in English", () => {
  assert.equal(languageName("sr"), "Serbian");
  assert.equal(languageName("de"), "German");
});

test("the label says what Whisper heard and how sure it was", () => {
  assert.equal(detectedLabel({ code: "hr", probability: 0.526195 }), "Detected · Croatian 53%");
  assert.equal(detectedLabel({ code: "en", probability: 0.381886 }), "Detected · English 38%");
});
```

- [ ] **Step 2: Run it, see it fail**

Run: `node --test src/lib/language.test.ts`
Expected: FAIL, cannot find module `./language.ts`.

- [ ] **Step 3: Implement**

`src/lib/language.ts`:

```ts
/**
 * What the language dialog offers, and what each choice sends to Whisper.
 *
 * Whisper cannot tell Serbian from Croatian or Bosnian -- the owner's own
 * Serbian came back as "hr" -- and `-l sr` is the one that writes ekavian, so
 * all three are Serbian here and send "sr".
 */

const SERBIAN = ["sr", "hr", "bs"];
const ALWAYS = ["sr", "en"];

/** The code a guess selects: Serbian for any of the three, else as heard. */
export function choiceFor(code: string): string {
  return SERBIAN.includes(code) ? "sr" : code;
}

/** Serbian and English always; a third button for anything else Whisper names. */
export function choicesFor(detected: string | null): string[] {
  if (!detected) return ALWAYS;
  const choice = choiceFor(detected);
  return ALWAYS.includes(choice) ? ALWAYS : [...ALWAYS, choice];
}

/** "hr" -> "Croatian". The code itself when the name is unknown. */
export function languageName(code: string): string {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** "Detected · Croatian 53%" -- uppercased by the stylesheet, not here. */
export function detectedLabel(detected: { code: string; probability: number }): string {
  return `Detected · ${languageName(detected.code)} ${Math.round(detected.probability * 100)}%`;
}
```

- [ ] **Step 4: Run it, see it pass**

Run: `node --test src/lib/language.test.ts`
Expected: 4 pass, 0 fail.

- [ ] **Step 5: Commit**

```bash
git add src/lib/language.ts src/lib/language.test.ts
git commit -m "Decide what the language dialog offers"
```

---

### Task 4: The language travels with the file

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/queue.ts`
- Modify: `src/lib/useJobs.ts`

- [ ] **Step 1: `api.ts`**

After the `LocalFileRequest` type, add:

```ts
/** What Whisper heard in a file. Mirrors `Detected` in services/language.rs. */
export type DetectedLanguage = {
  /** Whisper's code: "sr", "hr", "en", ... */
  code: string;
  /** 0-1, how sure it was. */
  probability: number;
};
```

After `transcribeFile`, add:

```ts
/** Which language a file is spoken in. A guess, for the dialog to suggest. */
export function detectLanguage(
  path: string,
  whisperModelPath: string | null,
): Promise<DetectedLanguage> {
  return invoke("detect_language", { request: { path, whisperModelPath } });
}
```

- [ ] **Step 2: `queue.ts`**

In `QueueItem`, after `named?: boolean;`, add:

```ts
  /** A file's spoken language, as Whisper's code. Absent lets Whisper guess. */
  language?: string | null;
```

Replace `fileItem` with:

```ts
export function fileItem(path: string, language: string | null = null): QueueItem {
  return {
    id: crypto.randomUUID(),
    kind: "file",
    label: path.split("/").pop() ?? path,
    target: path,
    status: "waiting",
    language,
  };
}
```

- [ ] **Step 3: `useJobs.ts`**

In `runOne`, replace `language: null,` with:

```ts
        language: item.language ?? null,
```

- [ ] **Step 4: Typecheck**

Run: `npm run build:vite`
Expected: clean (`fileItem(path)` callers still compile: the language defaults to null).

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/queue.ts src/lib/useJobs.ts
git commit -m "Carry a file's language to Whisper"
```

---

### Task 5: The dialog

**Files:**
- Create: `src/components/LanguagePrompt.tsx`
- Modify: `src/lib/motion.ts`
- Modify: `src/styles.css`

- [ ] **Step 1: `motion.ts`** -- after `cardEnter`, add:

```ts
/** A dialog: it opens bottom to top, like a card's cover. */
export function sheetEnter(el: HTMLElement) {
  if (stillPreferred()) return;
  el.animate(
    [
      { clipPath: "inset(100% 0 0 0 round .5rem)" },
      { clipPath: "inset(0 0 0 0 round .5rem)" },
    ],
    { duration: 700, easing: WIPE, fill: "backwards" },
  );
}
```

- [ ] **Step 2: `src/components/LanguagePrompt.tsx`**

```tsx
import { useEffect, useRef, useState } from "react";
import { detectLanguage, type DetectedLanguage } from "../lib/api";
import { choiceFor, choicesFor, detectedLabel, languageName } from "../lib/language";
import { rowEnter, sheetEnter } from "../lib/motion";
import { Segmented } from "./Controls";

export type Confirmed = { path: string; language: string };

type Props = {
  /** Files waiting to be transcribed, in the order they arrived. */
  paths: string[];
  whisperModelPath: string | null;
  /** What a row selects when Whisper cannot say: the Caption language. */
  fallback: string;
  onConfirm: (files: Confirmed[]) => void;
  onCancel: () => void;
};

type Row = {
  path: string;
  /** Absent while Whisper listens; null when it could not tell. */
  detected?: DetectedLanguage | null;
  choice: string;
  /** Chosen by hand: a guess that arrives later must not undo it. */
  picked: boolean;
};

/**
 * Which language each file is spoken in, asked before it is transcribed.
 *
 * Whisper's guess is shown and selected, never trusted: on 2026-10-24 Serbian
 * over music came back as English. One file is heard at a time, so two
 * Whisper processes never compete for the GPU.
 */
export function LanguagePrompt({ paths, whisperModelPath, fallback, onConfirm, onCancel }: Props) {
  const [rows, setRows] = useState<Row[]>([]);
  const sheet = useRef<HTMLDivElement>(null);
  const go = useRef<HTMLButtonElement>(null);
  // StrictMode runs effects twice; a file is still only heard once.
  const asked = useRef(new Set<string>());

  // A file that arrives while the dialog is open becomes another row.
  useEffect(() => {
    setRows((prev) => [
      ...prev,
      ...paths
        .filter((path) => !prev.some((row) => row.path === path))
        .map((path) => ({ path, choice: fallback, picked: false })),
    ]);
  }, [paths, fallback]);

  const listening = rows.find((row) => row.detected === undefined)?.path ?? null;

  useEffect(() => {
    if (!listening || asked.current.has(listening)) return;
    asked.current.add(listening);
    detectLanguage(listening, whisperModelPath)
      .then(
        (detected): DetectedLanguage | null => detected,
        () => null,
      )
      .then((detected) =>
        setRows((prev) =>
          prev.map((row) =>
            row.path !== listening
              ? row
              : {
                  ...row,
                  detected,
                  choice: row.picked || !detected ? row.choice : choiceFor(detected.code),
                },
          ),
        ),
      );
  }, [listening, whisperModelPath]);

  useEffect(() => {
    if (sheet.current) sheetEnter(sheet.current);
    go.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  function pick(path: string, choice: string) {
    setRows((prev) =>
      prev.map((row) => (row.path === path ? { ...row, choice, picked: true } : row)),
    );
  }

  return (
    <div className="prompt">
      <div
        ref={sheet}
        className="prompt__sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby="prompt-title"
      >
        <h2 id="prompt-title" className="prompt__title">
          Which language is spoken?
        </h2>
        <ul className="prompt__rows">
          {rows.map((row, index) => (
            <PromptRow
              key={row.path}
              row={row}
              index={index}
              onPick={(choice) => pick(row.path, choice)}
            />
          ))}
        </ul>
        <div className="prompt__actions">
          <button type="button" className="ghost ghost--small" onClick={onCancel}>
            Cancel
          </button>
          <button
            ref={go}
            type="button"
            className="cta cta--small"
            onClick={() => onConfirm(rows.map(({ path, choice }) => ({ path, language: choice })))}
          >
            Transcribe
          </button>
        </div>
      </div>
    </div>
  );
}

function PromptRow({
  row,
  index,
  onPick,
}: {
  row: Row;
  index: number;
  onPick: (choice: string) => void;
}) {
  const ref = useRef<HTMLLIElement>(null);
  const name = row.path.split("/").pop() ?? row.path;

  // Once, on arrival: a row joining an open dialog rises in on its own.
  useEffect(() => {
    if (ref.current) rowEnter(ref.current, index);
  }, []);

  const state =
    row.detected === undefined
      ? "Listening…"
      : row.detected === null
        ? "Not detected"
        : detectedLabel(row.detected);

  return (
    <li ref={ref} className="prompt__row">
      <span className="prompt__name" data-part>
        {name}
      </span>
      <span className="prompt__state" data-part>
        {state}
      </span>
      <span className="prompt__choice" data-part>
        <Segmented<string>
          label={`Language spoken in ${name}`}
          value={row.choice}
          onChange={onPick}
          options={choicesFor(row.detected?.code ?? null).map((code) => ({
            value: code,
            label: languageName(code),
          }))}
        />
      </span>
    </li>
  );
}
```

- [ ] **Step 3: `styles.css`** -- after the `.update__actions` rule (end of "a new version"), add:

```css
/* ---------- which language ------------------------------------------------ */

/* Over every screen: a recording can stop while you are anywhere. */
.prompt {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: grid;
  place-items: center;
  padding: 2rem;
  background: color-mix(in srgb, var(--bg-deep) 72%, transparent);
}

.prompt__sheet {
  width: min(40rem, 100%);
  max-height: calc(100vh - 4rem);
  overflow-y: auto;
  padding: 1.75rem 1.75rem 1.25rem;
  border: 1px solid var(--line-2);
  border-radius: 0.5rem;
  background: var(--card);
}

.prompt__title {
  margin: 0 0 1rem;
  font: 400 1.75rem/1.1 var(--font-serif);
  color: var(--bone);
}

.prompt__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}

.prompt__row {
  position: relative;
  display: grid;
  grid-template-columns: 1fr auto;
  grid-template-areas:
    "name choice"
    "state choice";
  align-items: center;
  gap: 0.375rem 1.5rem;
  padding: 0.875rem 0;
}

/* The hairline under each row; it draws from the left as the row arrives. */
.prompt__row::after {
  content: "";
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 1px;
  background: var(--line);
  transform-origin: left;
}

.prompt__name {
  grid-area: name;
  font: 500 0.9375rem/1.3 var(--font-sans);
  color: var(--bone);
  overflow-wrap: anywhere;
}

/* Only read, never clicked: mono, uppercase, dim. */
.prompt__state {
  grid-area: state;
  font: 500 0.625rem/1 var(--font-mono);
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--caption);
}

.prompt__choice {
  grid-area: choice;
}

.prompt__actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1.25rem;
}
```

- [ ] **Step 4: Typecheck**

Run: `npm run build:vite`
Expected: clean. (The dialog is not mounted yet; that is Task 6.)

- [ ] **Step 5: Commit**

```bash
git add src/components/LanguagePrompt.tsx src/lib/motion.ts src/styles.css
git commit -m "Draw the dialog that asks which language is spoken"
```

---

### Task 6: Every file passes through the dialog

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/ConvertView.tsx`
- Modify: `src/components/RecordView.tsx`

- [ ] **Step 1: `App.tsx` -- state and the one way in**

Add the import next to the other components:

```ts
import { LanguagePrompt } from "./components/LanguagePrompt";
```

After `const [libraryVersion, setLibraryVersion] = useState(0);`, add:

```ts
  /** Files waiting to be told their language before they join the queue. */
  const [asking, setAsking] = useState<string[]>([]);

  // Every way a file reaches the queue comes through here, so every file is
  // asked about. A functional update: the drop listener and the recorder hold
  // on to the first render's copy of this function.
  function transcribe(paths: string[]) {
    if (paths.length === 0) return;
    setAsking((prev) => [...prev, ...paths.filter((path) => !prev.includes(path))]);
  }
```

- [ ] **Step 2: `App.tsx` -- the two automatic ways in**

In `useRecorder({ onSaved })`, replace `void jobs.enqueue([fileItem(path)]);` with `transcribe([path]);` and the comment above it with `// A finished recording is asked about like any dropped file.`

In the drag-drop effect, replace `void jobs.enqueue(event.payload.paths.map((path) => fileItem(path)));` with `transcribe(event.payload.paths);` and its comment `// Files dropped anywhere on the window start straight away.` with `// Files dropped anywhere on the window are asked about first.`

- [ ] **Step 3: `App.tsx` -- hand the way in to the views, render the dialog**

In `<ConvertView ...>`, add the prop `onTranscribe={transcribe}`.

Replace `<RecordView recorder={recorder} jobs={jobs} />` with `<RecordView recorder={recorder} onTranscribe={transcribe} />`.

After `</main>`, before the closing `</div>` of `.app`, add:

```tsx
      {asking.length > 0 && (
        <LanguagePrompt
          paths={asking}
          whisperModelPath={settings.whisperModelPath || null}
          fallback={settings.transcriptLangs[0] ?? "sr"}
          onConfirm={(files) => {
            setAsking([]);
            void jobs.enqueue(files.map((file) => fileItem(file.path, file.language)));
          }}
          onCancel={() => setAsking([])}
        />
      )}
```

- [ ] **Step 4: `ConvertView.tsx`**

Add to `Props`, after `onClearLinks: () => void;`:

```ts
  /** Transcribe files on disk: they are asked about before they queue. */
  onTranscribe: (paths: string[]) => void;
```

Add `onTranscribe,` to the destructured props after `onClearLinks,`. In `chooseFiles`, replace `if (paths.length > 0) void jobs.enqueue(paths.map((path) => fileItem(path)));` with `onTranscribe(paths);`. Change the queue import to `import { urlItem, type Pasted, type PendingLink } from "../lib/queue";`.

- [ ] **Step 5: `RecordView.tsx`**

Replace the `Props` type's `jobs: Jobs;` with:

```ts
  /** Transcribe a saved recording: it is asked about before it queues. */
  onTranscribe: (paths: string[]) => void;
```

Change the signature to `export function RecordView({ recorder, onTranscribe }: Props)`. Replace `onClick={() => void jobs.enqueue([fileItem(path)])}` with `onClick={() => onTranscribe([path])}`. Delete the now-unused imports `import { fileItem } from "../lib/queue";` and `import type { Jobs } from "../lib/useJobs";`.

- [ ] **Step 6: Typecheck**

Run: `npm run build:vite`
Expected: clean. If `tsc` reports an unused import, delete it -- nothing else.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/components/ConvertView.tsx src/components/RecordView.tsx
git commit -m "Ask which language is spoken before any file is transcribed"
```

---

### Task 7: Write it down

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: `CHANGELOG.md`** -- under `## Unreleased`, `### New`, add:

```markdown
- Before transcribing a recording or a file, the app asks which language is
  spoken, with its own guess already chosen.
```

- [ ] **Step 2: `CLAUDE.md`**

In "Project structure", in the services list, after `` `model` fetching a Whisper model `` add `` · `language` hearing which language a file is in``.

In "Current status", in the "Working:" sentence, before "finding a word in an open document" add "asking which language a file is spoken in before transcribing it, ".

- [ ] **Step 3: Check the changelog still parses**

Run: `python3 scripts/changelog.py text Unreleased`
Expected: the New line and the two Fixed lines, no error.

- [ ] **Step 4: Commit**

```bash
git add CHANGELOG.md CLAUDE.md
git commit -m "Write down the language dialog"
```

---

### Task 8: Prove it on the real thing

- [x] **Step 1: test-runner** -- `cargo test` (116), `npm run build:vite`, `python3 scripts/test_changelog.py`, `node --test src/lib/releaseNotes.test.ts src/lib/language.test.ts`.
- [x] **Step 2: reviewer** on the diff since `00ad327`. Fix what it finds.
  Fixed: Transcribe now waits for every row to have a guess or a hand choice
  (it was focused and live while rows still read LISTENING), a guess gives up
  after 60 s, and the fallback is clamped to Serbian or English.
- [x] **Step 3: Ask the owner**, then `npm run install:app` (no job may be running; the script refuses if one is).
- [ ] **Step 4: The music recording** -- not run as written. Instead a new
  recording of music (`16-18`) went through the dialog and came back
  `whisper (sr)`, with the song's English lyrics, as sung.
  Original step: -- the owner clicks Transcribe on `Recording 2026-10-24 14-50.mp3` in Record. Expected: the dialog says `DETECTED · ENGLISH 38%` (or similar) with English selected; the owner picks Serbian; the new `.transcript.md` beside it reads `Transcript from: whisper (sr)` and is Serbian text.
- [x] **Step 5: A clean recording** -- the owner records ten seconds of Serbian on the mic and stops. Expected: the dialog opens on its own, reads `DETECTED · CROATIAN …` with Serbian selected, and Transcribe produces Serbian.
  Result (`16-20`): `whisper (sr)`, "Ok, ako pričam sad, ne znam šta da kažem…".
- [ ] **Step 6: Two at once** -- not run before 0.2.0. -- the owner drops two files on the window; one dialog, two rows, heard one after the other. Cancel queues nothing.
