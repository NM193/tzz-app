# A name the library already has -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When Start (or Transcribe in the language dialog) would send a job into a library folder that already holds a result, or into the same folder as another job in the batch, one dialog asks once per job: New name, Replace (everything but the notes), or Skip. Then the queue runs without stopping.

**Architecture:** A new Rust service, `services/names.rs`, owns what a folder is called (`folder_name`, `suggest`), what counts as taken (`holds`, `source_inside`), the batch check (`clashes`) and the clearing (`clear_for_replace`). The jobs in `commands/media.rs` name their folders through it, so the check and the job cannot disagree. A thin `library_clashes` command exposes the check. On the frontend, `App.tsx` gets one way into the queue (`admit`): check, open `NamePrompt` if anything clashes, then `jobs.enqueue`. The dialog's logic lives in a pure module, `src/lib/naming.ts`, tested with `node --test`.

**Tech Stack:** Rust (std::fs, serde), Tauri v2 commands, React 18 + TypeScript, `node --test` for pure TS logic.

**Spec:** `docs/superpowers/specs/2026-10-30-name-taken-design.md`
**Prototype:** `.superpowers/prototype/tzz-name-taken.html`

**Two departures from the spec's wording, both internal:**
- The logic goes in a new `services/names.rs`, not in `services/library.rs`. `library.rs` lists the library for the Library screen. Naming and clashing is a separate job with four times the code, and it would double that file.
- A candidate has no `kind` field. A file is the item that carries `path`.

**Working rules for this repo (from CLAUDE.md):**
- Do not edit Rust while `npm run dev` runs a job. The watcher restarts the app and kills it.
- Rust structs facing the frontend are `#[serde(rename_all = "camelCase")]`, mirrored by hand in `src/lib/api.ts`.
- Commands are thin; logic lives in services.
- Every colour lives at the top of `src/styles.css`. Reuse tokens and existing classes.
- Commit on `main` after each task. Never push.

---

## Files

| File | What it does |
|---|---|
| `src-tauri/src/services/names.rs` (new) | folder names, suggestions, what a folder holds, the batch check, clearing for Replace; tests |
| `src-tauri/src/services/mod.rs` | `pub mod names;` |
| `src-tauri/src/services/library.rs` | `VIDEO` and `modified_at` become `pub(super)` for `names.rs` |
| `src-tauri/src/commands/media.rs` | jobs name folders through `names`; `replace` and `name` on the requests |
| `src-tauri/src/commands/library.rs` | `library_clashes` command |
| `src-tauri/src/lib.rs` | registers the command |
| `src/lib/api.ts` | `Candidate`, `Holds`, `Clash`, `libraryClashes`; `replace`/`name` on requests |
| `src/lib/queue.ts` | `QueueItem.videoId/replace/name`, `PendingLink.videoId` |
| `src/lib/useJobs.ts` | `runOne` sends `replace` and `name` |
| `src/lib/naming.ts` (new) | the dialog's logic, pure |
| `src/lib/naming.test.ts` (new) | `node --test` for the above |
| `src/components/Controls.tsx` | `Segmented` options can be disabled |
| `src/components/NamePrompt.tsx` (new) | the dialog |
| `src/styles.css` | the row's field, the refused state, a disabled segment |
| `src/components/ConvertView.tsx` | Start hands its batch to `onStart` |
| `src/App.tsx` | `admit`, the dialog, `videoId` on pills |
| `CHANGELOG.md`, `CLAUDE.md` | the New line; `names` in the services list |

---

### Task 1: Folder names and suggestions

**Files:**
- Create: `src-tauri/src/services/names.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Register the module**

In `src-tauri/src/services/mod.rs`, add the line between `pub mod mp3;` and `pub mod notes;`:

```rust
pub mod names;
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/services/names.rs` with only the module doc, imports and tests:

```rust
//! What a job's folder in the library is called, and whether it is taken.
//!
//! The job and the check before Start both name folders here, so the two
//! cannot disagree. Taken means the folder already holds a result -- an MP3
//! or a transcript -- not merely that it exists: a failed run leaves a folder
//! of leftovers, and running it again must not be asked about.

use std::path::Path;

use super::combine;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_what_they_were_before() {
        // What job_folder and library_folder made before this module existed:
        // the whole name, `[id]` included, cut to 120 characters and trimmed.
        let old = |name: &str| name.chars().take(120).collect::<String>().trim().to_string();
        let title = "Lumos V2.1 Crash Course (Webflow Framework)";
        assert_eq!(folder_name(title, Some("GPR123")), old(&format!("{title} [GPR123]")));
        assert_eq!(folder_name("A/B testing: what works", Some("x1")), "A-B testing- what works [x1]");
        assert_eq!(folder_name("Lecture 1", None), "Lecture 1");
        let long = "x".repeat(200);
        assert_eq!(folder_name(&long, None), old(&long));
    }

    #[test]
    fn a_long_name_keeps_its_id() {
        let name = folder_name(&"y".repeat(200), Some("abc123"));
        assert!(name.ends_with(" [abc123]"), "{name}");
        assert_eq!(name.chars().count(), 120);
    }

    #[test]
    fn a_long_name_keeps_its_number() {
        let long = "z".repeat(200);
        let (two, three) = (suggest(&long, None, 2), suggest(&long, None, 3));
        assert!(two.ends_with(" (2)") && three.ends_with(" (3)"));
        assert_ne!(folder_name(&two, None), folder_name(&three, None));
    }

    #[test]
    fn a_suggestion_comes_back_unchanged() {
        let (w, v) = ("w".repeat(300), "v".repeat(300));
        let cases: [(&str, Option<&str>); 4] =
            [("Lecture 1", None), (&w, None), (&v, Some("id9")), ("Talk", Some("id9"))];
        for (name, id) in cases {
            for n in [2, 9, 10, 123] {
                let suggestion = suggest(name, id, n);
                let expected = format!("{suggestion}{}", id_part(id));
                assert_eq!(folder_name(&suggestion, id), expected, "{suggestion}");
            }
        }
    }

    #[test]
    fn a_suggestion_is_numbered_and_trimmed() {
        assert_eq!(suggest("Lecture 1", None, 2), "Lecture 1 (2)");
        // A cut that ends on a space does not leave two before the number.
        let spaced = format!("{} tail", "a".repeat(115));
        assert!(!suggest(&spaced, None, 2).contains("  ("));
    }

    #[test]
    fn hidden_or_empty_names_cannot_be_folders() {
        for bad in ["", "   ", ".", "..", ".hidden"] {
            assert!(!usable(bad), "{bad:?}");
        }
        for good in ["Lecture 1", "A/B", "1. Intro"] {
            assert!(usable(good), "{good:?}");
        }
    }

    #[test]
    fn a_file_is_named_by_its_stem() {
        assert_eq!(file_title(Path::new("/x/1. Intro.mp4")), "1. Intro");
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd src-tauri && cargo test names::`
Expected: compile errors, `cannot find function folder_name` (and `suggest`, `usable`, `file_title`, `id_part`).

- [ ] **Step 4: Write the implementation**

In `names.rs`, between the imports and `#[cfg(test)]`, add:

```rust
/// The longest a folder's name gets, `[id]` and `(n)` included. It was the
/// limit before there was a check, and it keeps a name, plus what the files
/// inside add to it, clear of the 255-byte filename limit.
const LIMIT: usize = 120;

/// Sanitised, cut to `room` characters, trimmed.
fn cut(name: &str, room: usize) -> String {
    combine::sanitise(name).chars().take(room).collect::<String>().trim().to_string()
}

fn id_part(id: Option<&str>) -> String {
    id.map(|id| format!(" [{id}]")).unwrap_or_default()
}

/// The folder a job writes into: the name cut to fit, then ` [id]` for a
/// link. Cut first, so a long name keeps its `[id]` -- `thumb.rs` finds the
/// cover through it.
pub fn folder_name(name: &str, id: Option<&str>) -> String {
    let id = id_part(id);
    format!("{}{id}", cut(name, LIMIT.saturating_sub(id.chars().count())))
}

/// `<name> (n)`, cut so the whole of it fits as `folder_name` would cut it.
/// It comes back through `folder_name` unchanged, which is what lets it
/// travel to the job as a plain name and keep its number.
pub fn suggest(name: &str, id: Option<&str>, n: usize) -> String {
    let number = format!(" ({n})");
    let room = LIMIT.saturating_sub(id_part(id).chars().count() + number.chars().count());
    format!("{}{number}", cut(name, room)).trim().to_string()
}

/// A name that can be a folder in the library: something is left after
/// sanitising, and it is not hidden. `..` would leave the library, and a
/// leading dot would hide the folder from the Library screen.
pub fn usable(name: &str) -> bool {
    let base = cut(name, LIMIT);
    !base.is_empty() && !base.starts_with('.')
}

/// A file's name without its extension, as its library folder is named.
pub fn file_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string())
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd src-tauri && cargo test names::`
Expected: 7 passed. Dead-code warnings for the `pub fn`s are expected until Task 4 uses them.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/names.rs src-tauri/src/services/mod.rs
git commit -m "Name library folders in one place, cut before the id and the number"
```

---

### Task 2: What a folder holds, and clearing it for Replace

**Files:**
- Modify: `src-tauri/src/services/names.rs`
- Modify: `src-tauri/src/services/library.rs:33` (`VIDEO`) and `:96` (`modified_at`)

- [ ] **Step 1: Open two helpers in `library.rs` to the sibling module**

In `src-tauri/src/services/library.rs`, change

```rust
const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov"];
```

to

```rust
pub(super) const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov"];
```

and change `fn modified_at(path: &Path) -> u64 {` to `pub(super) fn modified_at(path: &Path) -> u64 {`.

- [ ] **Step 2: Write the failing tests**

In `names.rs`, change the imports to:

```rust
use std::path::Path;

use serde::Serialize;

use super::combine;
use super::library;
```

and add to the `tests` module:

```rust
    fn temp_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("tzz-names-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn folder_with(root: &Path, name: &str, files: &[&str]) -> std::path::PathBuf {
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        for file in files {
            std::fs::write(folder.join(file), b"x").unwrap();
        }
        folder
    }

    #[test]
    fn a_result_is_an_mp3_or_a_transcript() {
        let root = temp_root("result");
        let result = |folder: std::path::PathBuf| holds(&folder).has_result();
        assert!(result(folder_with(&root, "a", &["a.mp3"])));
        assert!(result(folder_with(&root, "b", &["b.md"])));
        assert!(result(folder_with(&root, "c", &["c.pdf"])));
        // What a failed run leaves, a kept video, notes: none is a result.
        assert!(!result(folder_with(&root, "d", &["d.webm", "d.en-orig.vtt", "poster.jpg"])));
        assert!(!result(folder_with(&root, "e", &["e.notes.md"])));
        assert!(!result(root.join("missing")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn says_what_a_folder_holds() {
        let root = temp_root("holds");
        let folder = folder_with(
            &root,
            "L [id]",
            &["L [id].mp3", "L [id].md", "L [id].notes.md", "L [id].screens.json", "L [id].mp4"],
        );
        let found = holds(&folder);
        assert!(found.audio && found.transcript && found.notes && found.screens && found.video);
        assert!(found.modified > 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_inside_its_own_folder_is_seen_whatever_the_case() {
        let root = temp_root("inside");
        let folder = folder_with(&root, "Lecture 1", &["Lecture 1.mp3"]);
        assert!(source_inside(&folder.join("Lecture 1.mp3"), &folder));
        // APFS ignores case: the same folder spelled another way.
        assert!(source_inside(&folder.join("Lecture 1.mp3"), &root.join("lecture 1")));
        assert!(!source_inside(Path::new("/tmp/elsewhere/Lecture 1.mp3"), &folder));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_keeps_the_notes_and_nothing_else() {
        let root = temp_root("clear");
        let folder =
            folder_with(&root, "L", &["L.mp3", "L.md", "L.notes.md", "L.screens.json", "poster.jpg"]);
        clear_for_replace(&root, &folder).unwrap();
        let left: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["L.notes.md".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_touches_nothing_outside_the_library() {
        let root = temp_root("outside");
        let elsewhere = temp_root("outside-elsewhere");
        let folder = folder_with(&elsewhere, "L", &["L.mp3"]);
        assert!(clear_for_replace(&root, &folder).is_err());
        assert!(folder.join("L.mp3").exists());
        // Nor anything deeper than one level, nor the library itself.
        let nested = folder_with(&root, "a/b", &["b.mp3"]);
        assert!(clear_for_replace(&root, &nested).is_err());
        assert!(clear_for_replace(&root, &root).is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd src-tauri && cargo test names::`
Expected: compile errors, `cannot find function holds` (and `source_inside`, `clear_for_replace`).

- [ ] **Step 4: Write the implementation**

In `names.rs`, after `file_title`, add:

```rust
/// What a folder holds, so the dialog can say what Replace would take.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Holds {
    /// An MP3.
    pub audio: bool,
    /// A transcript: markdown other than the notes, or a PDF.
    pub transcript: bool,
    pub video: bool,
    /// The cache of what was read off the screen.
    pub screens: bool,
    pub notes: bool,
    /// Seconds since the epoch: when anything in it last changed.
    pub modified: u64,
}

impl Holds {
    /// Taken: the folder holds an MP3 or a transcript. Leftovers of a
    /// failed run, a kept video, or notes alone do not count.
    pub fn has_result(&self) -> bool {
        self.audio || self.transcript
    }
}

pub fn holds(folder: &Path) -> Holds {
    let mut found = Holds::default();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let ext = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        if name.ends_with(".notes.md") {
            found.notes = true;
        } else if name.ends_with(".screens.json") {
            found.screens = true;
        } else if ext == "mp3" {
            found.audio = true;
        } else if ext == "md" || ext == "pdf" {
            found.transcript = true;
        } else if library::VIDEO.contains(&ext) {
            found.video = true;
        }
        found.modified = found.modified.max(library::modified_at(&path));
    }
    found
}

/// Whether two paths name one folder. By inode, as `mp3.rs` compares files:
/// the volume ignores case, so "Lecture 1" and "lecture 1" are one folder.
fn same_dir(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// A file being made again from inside the folder it would go to: dropped
/// from its own library folder. Replace there would delete it unread.
pub fn source_inside(source: &Path, folder: &Path) -> bool {
    source.parent().is_some_and(|parent| same_dir(parent, folder))
}

/// Empty a folder for Replace. Every file goes but the notes, the one thing
/// in it the app cannot make again. Only a folder directly inside the
/// library is touched.
pub fn clear_for_replace(root: &Path, folder: &Path) -> Result<(), String> {
    if !folder.parent().is_some_and(|parent| same_dir(parent, root)) {
        return Err("That folder is not in the library.".to_string());
    }
    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("Could not clear the old results: {e}"))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let notes = entry.file_name().to_string_lossy().to_lowercase().ends_with(".notes.md");
        if path.is_file() && !notes {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Could not clear the old results: {e}"))?;
        }
    }
    Ok(())
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd src-tauri && cargo test names:: && cargo test library::`
Expected: 12 passed in `names`, and the 4 `library` tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/names.rs src-tauri/src/services/library.rs
git commit -m "Say what a library folder holds, and clear it for Replace but the notes"
```

---

### Task 3: The batch check

**Files:**
- Modify: `src-tauri/src/services/names.rs`

- [ ] **Step 1: Write the failing tests**

In `names.rs`, change the imports to:

```rust
use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::combine;
use super::library;
```

and add to the `tests` module:

```rust
    fn link(key: &str, title: &str, id: &str) -> Candidate {
        Candidate {
            key: key.into(),
            path: None,
            title: Some(title.into()),
            video_id: Some(id.into()),
            name: None,
        }
    }

    fn file(key: &str, path: &Path) -> Candidate {
        Candidate {
            key: key.into(),
            path: Some(path.to_string_lossy().into_owned()),
            title: None,
            video_id: None,
            name: None,
        }
    }

    #[test]
    fn a_finished_link_is_in_the_library() {
        let root = temp_root("link");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3", "Talk [abc].notes.md"]);
        let found = clashes(&root, &[link("1", "Talk", "abc"), link("2", "Other", "def")]);
        assert!(found[0].in_library && found[0].replaceable);
        assert_eq!(found[0].folder, "Talk [abc]");
        assert_eq!(found[0].suggestion.as_deref(), Some("Talk (2)"));
        assert!(found[0].holds.audio && found[0].holds.notes);
        assert!(!found[1].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_run_is_not_asked_about() {
        let root = temp_root("failed");
        folder_with(&root, "Talk [abc]", &["Talk [abc].webm", "poster.jpg"]);
        assert!(!clashes(&root, &[link("1", "Talk", "abc")])[0].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_link_without_an_id_is_not_checked() {
        let root = temp_root("noid");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3"]);
        let mut item = link("1", "Talk", "abc");
        item.video_id = None;
        let found = clashes(&root, &[item]);
        assert!(!found[0].is_taken());
        assert!(found[0].folder.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_files_of_one_name_clash_whatever_the_case() {
        let root = temp_root("batch");
        let found = clashes(
            &root,
            &[file("a", Path::new("/w1/Lecture 1.mp4")), file("b", Path::new("/w2/lecture 1.mp4"))],
        );
        // The library has neither: the first is free, the second is not.
        assert!(!found[0].is_taken());
        assert_eq!(found[1].same_as.as_deref(), Some("a"));
        assert!(!found[1].replaceable);
        assert_eq!(found[1].suggestion.as_deref(), Some("lecture 1 (2)"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn suggestions_skip_every_name_the_batch_uses() {
        let root = temp_root("suggest");
        folder_with(&root, "Lecture 1", &["Lecture 1.mp3"]);
        let found = clashes(
            &root,
            &[
                file("a", Path::new("/w1/Lecture 1.mp4")),
                file("b", Path::new("/w2/Lecture 1.mp4")),
                file("c", Path::new("/w3/Lecture 1 (2).mp4")),
            ],
        );
        // (2) belongs to c, further down; a takes (3) and b the next free.
        assert_eq!(found[0].suggestion.as_deref(), Some("Lecture 1 (3)"));
        assert_eq!(found[1].suggestion.as_deref(), Some("Lecture 1 (4)"));
        assert!(!found[2].is_taken());
        // Only the first of a folder's jobs may replace it.
        assert!(found[0].replaceable && !found[1].replaceable);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_dropped_from_its_own_folder_is_not_taken() {
        let root = temp_root("own");
        let folder = folder_with(&root, "Lecture 1", &["Lecture 1.mp3", "Lecture 1.md"]);
        assert!(!clashes(&root, &[file("a", &folder.join("Lecture 1.mp3"))])[0].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_is_not_offered_when_another_job_reads_from_the_folder() {
        let root = temp_root("reads");
        let folder = folder_with(&root, "Talk [abc]", &["Talk [abc].mp3", "Talk [abc].mp4"]);
        let found = clashes(
            &root,
            &[link("link", "Talk", "abc"), file("kept", &folder.join("Talk [abc].mp4"))],
        );
        assert!(found[0].in_library);
        assert!(!found[0].replaceable, "Replace would delete the video the next job reads");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_from_the_dialog_wins_and_is_judged() {
        let root = temp_root("chosen");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3"]);
        let mut renamed = link("1", "Talk", "abc");
        renamed.name = Some("Talk (2)".into());
        let found = clashes(&root, &[renamed]);
        assert_eq!(found[0].folder, "Talk (2) [abc]");
        assert!(!found[0].is_taken());

        let mut hidden = link("2", "Talk", "abc");
        hidden.name = Some(".secret".into());
        assert!(!clashes(&root, &[hidden])[0].usable);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_clash_names_the_job_it_clashes_with_even_below() {
        let root = temp_root("below");
        let mut renamed = file("row", Path::new("/w1/Lecture 1.mp4"));
        renamed.name = Some("Lecture 2".into());
        let found = clashes(&root, &[renamed, file("other", Path::new("/w2/Lecture 2.mp4"))]);
        assert_eq!(found[1].same_as.as_deref(), Some("row"));
        let _ = std::fs::remove_dir_all(&root);
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd src-tauri && cargo test names::`
Expected: compile errors, `cannot find struct Candidate` and `cannot find function clashes`.

- [ ] **Step 3: Write the implementation**

In `names.rs`, after `clear_for_replace`, add:

```rust
/// One job about to join the queue, as the check before Start sees it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The frontend's id for the job, handed back unchanged.
    pub key: String,
    /// A file on disk. Its stem names the folder unless `name` says otherwise.
    pub path: Option<String>,
    /// A link's title.
    pub title: Option<String>,
    /// A link's video id. A link without one is not checked.
    pub video_id: Option<String>,
    /// The name in the dialog's field, which wins over the others.
    pub name: Option<String>,
}

/// What the check found for one job. Mirrors `Clash` in `src/lib/api.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    pub key: String,
    /// The folder the job would write into; empty when it was not checked.
    pub folder: String,
    /// The library already has that folder, holding a result.
    pub in_library: bool,
    /// The key of an earlier job in this batch that writes into it too.
    pub same_as: Option<String>,
    /// Replace is on offer: the library has the folder, no earlier job
    /// writes into it, and no other job reads its file from there.
    pub replaceable: bool,
    /// The first free `<name> (n)`, when the folder is taken.
    pub suggestion: Option<String>,
    /// The name can be a folder at all.
    pub usable: bool,
    pub holds: Holds,
}

// The tests' shorthand. The app asks the same question in `naming.ts`.
#[cfg(test)]
impl Clash {
    fn is_taken(&self) -> bool {
        self.in_library || self.same_as.is_some()
    }
}

/// The name a job's folder is made from, and the id it carries.
fn naming(item: &Candidate) -> Option<(String, Option<&str>)> {
    if let Some(path) = item.path.as_deref() {
        let name = item.name.clone().unwrap_or_else(|| file_title(Path::new(path)));
        return Some((name, None));
    }
    let id = item.video_id.as_deref()?;
    let name = item.name.clone().or_else(|| item.title.clone())?;
    Some((name, Some(id)))
}

/// Every job of a batch against the library and against each other, in
/// order, one answer each.
///
/// Names compare case-folded, as APFS compares them. Suggestions stay clear
/// of every folder the batch uses, above or below, and of each other.
pub fn clashes(root: &Path, items: &[Candidate]) -> Vec<Clash> {
    let named: Vec<Option<(String, Option<&str>)>> = items.iter().map(naming).collect();
    let folders: Vec<Option<String>> = named
        .iter()
        .map(|n| n.as_ref().map(|(name, id)| folder_name(name, *id)))
        .collect();
    let mut used: HashSet<String> = folders.iter().flatten().map(|f| f.to_lowercase()).collect();

    items
        .iter()
        .enumerate()
        .map(|(at, item)| {
            let (Some((name, id)), Some(folder)) = (&named[at], &folders[at]) else {
                return Clash {
                    key: item.key.clone(),
                    folder: String::new(),
                    in_library: false,
                    same_as: None,
                    replaceable: false,
                    suggestion: None,
                    usable: true,
                    holds: Holds::default(),
                };
            };
            let dir = root.join(folder);
            let reads_from = |candidate: &Candidate| {
                candidate.path.as_deref().is_some_and(|p| source_inside(Path::new(p), &dir))
            };

            let lower = folder.to_lowercase();
            let same_as = (0..at)
                .find(|&k| folders[k].as_ref().is_some_and(|f| f.to_lowercase() == lower))
                .map(|k| items[k].key.clone());
            let found = holds(&dir);
            let in_library = !reads_from(item) && found.has_result();
            let read_by_another =
                items.iter().enumerate().any(|(k, other)| k != at && reads_from(other));

            let suggestion = (in_library || same_as.is_some()).then(|| {
                let free = (2..)
                    .map(|n| suggest(name, *id, n))
                    .find(|s| {
                        let f = folder_name(s, *id);
                        !used.contains(&f.to_lowercase()) && !root.join(&f).exists()
                    })
                    .expect("one of endlessly many numbers is free");
                used.insert(folder_name(&free, *id).to_lowercase());
                free
            });

            Clash {
                key: item.key.clone(),
                folder: folder.clone(),
                in_library,
                replaceable: in_library && same_as.is_none() && !read_by_another,
                same_as,
                suggestion,
                usable: usable(name),
                holds: found,
            }
        })
        .collect()
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd src-tauri && cargo test names::`
Expected: 21 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/names.rs
git commit -m "Check a batch of jobs against the library and against each other"
```

---

### Task 4: Jobs name their folders through `names`, Replace and a new name reach them, the command

**Files:**
- Modify: `src-tauri/src/commands/media.rs` (imports, `JobRequest`, `LocalFileRequest`, `run_job_inner`, `transcribe_file_inner`, `job_folder`, `library_folder`)
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Import `names` in `media.rs`**

After `use crate::services::mp3;` add:

```rust
use crate::services::names;
```

- [ ] **Step 2: Add the request fields**

In `JobRequest`, after the `title` field, add:

```rust
    /// The library had this folder and the owner chose to replace it:
    /// everything in it but the notes goes before the job starts.
    pub replace: Option<bool>,
```

In `LocalFileRequest`, after `audio_quality`, add:

```rust
    /// A name chosen in place of the file's own, because the library had
    /// that one. Names the folder, the files and the transcript.
    pub name: Option<String>,
    /// As for a download: clear the folder, all but the notes, first.
    pub replace: Option<bool>,
```

- [ ] **Step 3: Clear the folder for a link with Replace**

In `run_job_inner`, right after `let job_dir = job_folder(&out_dir, &title, &meta.id)?;`, add:

```rust
    // Cleared here, when the queue reaches this job, not when Replace was
    // chosen: a queue stopped before now leaves the old results alone.
    if request.replace.unwrap_or(false) {
        names::clear_for_replace(&out_dir, &job_dir)?;
    }
```

- [ ] **Step 4: Use the chosen name and Replace for a file**

In `transcribe_file_inner`, replace

```rust
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string());

    // The original stays exactly where it is, untouched. Everything this job
    // makes goes into a folder of its own in the library, as a download's does.
    let job_dir = library_folder(&out_dir, &combine::sanitise(&title))?;
    let _hollow = RemoveIfEmpty(&job_dir);
```

with

```rust
    // A name chosen in the dialog, because the library had the file's own.
    let chosen = request.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
    if chosen.is_some_and(|name| !names::usable(name)) {
        return Err("That name cannot be a folder in the library.".to_string());
    }
    let title = chosen.map(str::to_string).unwrap_or_else(|| names::file_title(&path));

    // The original stays exactly where it is, untouched. Everything this job
    // makes goes into a folder of its own in the library, as a download's does.
    let job_dir = library_folder(&out_dir, &names::folder_name(&title, None))?;
    let _hollow = RemoveIfEmpty(&job_dir);
    // Never the folder the file itself is in: Replace would delete it unread.
    if request.replace.unwrap_or(false) && !names::source_inside(&path, &job_dir) {
        names::clear_for_replace(&out_dir, &job_dir)?;
    }
```

- [ ] **Step 5: Name folders through `names`**

Replace `job_folder` and `library_folder` with:

```rust
fn job_folder(out_dir: &std::path::Path, title: &str, video_id: &str) -> Result<PathBuf, String> {
    library_folder(out_dir, &names::folder_name(title, Some(video_id)))
}
```

(keep the doc comment above `job_folder` as it is), and replace
`library_folder` together with its one-line doc comment with:

```rust
/// A folder of its own in the library, for a download or a file on disk.
/// `names::folder_name` has already cut the name to fit.
fn library_folder(out_dir: &std::path::Path, name: &str) -> Result<PathBuf, String> {
    let folder = out_dir.join(name);
    // Claimed before it is made, so a stop knows whether it was ours to remove.
    cancel::claim(&folder);
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not create a folder in the library: {e}"))?;
    Ok(folder)
}
```

- [ ] **Step 6: Add the command**

In `src-tauri/src/commands/library.rs`, after `use crate::services::library::{self, LibraryEntry};` add:

```rust
use crate::services::names::{self, Candidate, Clash};
```

and after `list_library` add:

```rust
/// Which of the jobs about to start would land in a folder the library
/// already has, or in one another of them uses. One answer per job, in order.
#[tauri::command]
pub fn library_clashes(out_dir: Option<String>, items: Vec<Candidate>) -> Result<Vec<Clash>, String> {
    let root = resolve_out_dir(out_dir.as_deref())?;
    Ok(names::clashes(&root, &items))
}
```

In `src-tauri/src/lib.rs`, after `commands::library::list_library,` add:

```rust
            commands::library::library_clashes,
```

- [ ] **Step 7: Run every Rust test**

Run: `cd src-tauri && cargo test`
Expected: all pass, including `names_a_folder_after_the_video_and_keeps_it_openable` and `a_slash_in_the_title_does_not_become_a_folder` in `media.rs`, which pin today's folder names. No warnings from `names.rs`: everything in it is now used by the jobs or the command, and `Clash::is_taken` exists only under `#[cfg(test)]`.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/media.rs src-tauri/src/commands/library.rs src-tauri/src/lib.rs
git commit -m "Jobs name folders through names; Replace and a new name reach them"
```

---

### Task 5: Frontend types and the job requests

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/queue.ts`
- Modify: `src/lib/useJobs.ts` (`runOne`)

- [ ] **Step 1: Mirror the request fields**

In `src/lib/api.ts`, in `JobRequest`, after `title: string | null;` add:

```ts
  /** Clear the folder, all but the notes, before the job starts. */
  replace: boolean;
```

In `LocalFileRequest`, after `audioQuality: AudioQuality;` add:

```ts
  /** A name in place of the file's own, for its folder, files and title. */
  name: string | null;
  /** Clear the folder, all but the notes, before the job starts. */
  replace: boolean;
```

- [ ] **Step 2: Mirror the check**

In `src/lib/api.ts`, after `listLibrary`, add:

```ts
/** A job about to join the queue. Mirrors `Candidate` in services/names.rs. */
export type Candidate = {
  /** The queue item's id, handed back unchanged. */
  key: string;
  /** A file on disk; its stem names the folder unless `name` is given. */
  path?: string | null;
  /** A link's title. */
  title?: string | null;
  /** A link's video id. A link without one is not checked. */
  videoId?: string | null;
  /** The name in the dialog's field. */
  name?: string | null;
};

/** What a library folder holds. Mirrors `Holds` in services/names.rs. */
export type Holds = {
  audio: boolean;
  transcript: boolean;
  video: boolean;
  screens: boolean;
  notes: boolean;
  /** Seconds since the epoch. */
  modified: number;
};

/** What the check found for one job. Mirrors `Clash` in services/names.rs. */
export type Clash = {
  key: string;
  /** Empty when the job was not checked. */
  folder: string;
  inLibrary: boolean;
  /** The key of an earlier job in the batch with the same folder. */
  sameAs: string | null;
  replaceable: boolean;
  suggestion: string | null;
  usable: boolean;
  holds: Holds;
};

export function libraryClashes(outDir: string | null, items: Candidate[]): Promise<Clash[]> {
  return invoke("library_clashes", { outDir, items });
}
```

- [ ] **Step 3: Carry the id and the answers on queue items and pills**

In `src/lib/queue.ts`, in `QueueItem`, after `wantTranscript?: boolean;` add:

```ts
  /** A link's video id, when known, so the library can be checked for it. */
  videoId?: string;
  /** Clear the folder's old results first, all but the notes. */
  replace?: boolean;
  /** A file's new name, for its folder, files and title. A link's new name goes in `label`. */
  name?: string;
```

In `PendingLink`, after `file?: boolean;` add:

```ts
  /** The video id, once the title has been read or the playlist unpacked. */
  videoId?: string;
```

- [ ] **Step 4: Send them**

In `src/lib/useJobs.ts`, in `runOne`, in the `transcribeFile({...})` call, after `audioQuality: s.audioQuality,` add:

```ts
        name: item.name ?? null,
        replace: item.replace ?? false,
```

and in the `runJob({...})` call, after `title: item.named ? item.label : null,` add:

```ts
      replace: item.replace ?? false,
```

- [ ] **Step 5: Typecheck**

Run: `npm run build:vite`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add src/lib/api.ts src/lib/queue.ts src/lib/useJobs.ts
git commit -m "Carry Replace, a new name and the video id through the queue"
```

---

### Task 6: The dialog's logic, pure

**Files:**
- Create: `src/lib/naming.ts`
- Create: `src/lib/naming.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/lib/naming.test.ts`:

```ts
// Run: node --test src/lib/naming.test.ts
// Node 22 runs TypeScript directly; nothing needs installing.
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Clash, Holds } from "./api";
import type { QueueItem } from "./queue";
import {
  asRun,
  fromFolder,
  refusalLine,
  refused,
  replaceLine,
  rowsFrom,
  stateLine,
  toCandidates,
  type NameRow,
} from "./naming.ts";

const file = (id: string, target: string): QueueItem => ({
  id,
  kind: "file",
  label: target.split("/").pop() ?? target,
  target,
  status: "waiting",
});

const link = (id: string, title: string, videoId?: string): QueueItem => ({
  id,
  kind: "url",
  label: title,
  target: `https://youtu.be/${id}`,
  status: "waiting",
  named: true,
  videoId,
});

const none: Holds = { audio: false, transcript: false, video: false, screens: false, notes: false, modified: 0 };

const clash = (key: string, patch: Partial<Clash> = {}): Clash => ({
  key,
  folder: key,
  inLibrary: false,
  sameAs: null,
  replaceable: false,
  suggestion: null,
  usable: true,
  holds: none,
  ...patch,
});

test("a link is checked by its title and id, a file by its path", () => {
  assert.deepEqual(toCandidates([link("a", "Talk", "abc"), file("b", "/x/Lecture 1.mp4")]), [
    { key: "a", title: "Talk", videoId: "abc" },
    { key: "b", path: "/x/Lecture 1.mp4", name: null },
  ]);
});

test("rows are the jobs the first check found taken, New name chosen", () => {
  const rows = rowsFrom([
    clash("a", { inLibrary: true, suggestion: "Talk (2)" }),
    clash("b"),
    clash("c", { sameAs: "a", suggestion: "Talk (3)" }),
  ]);
  assert.deepEqual(rows, [
    { key: "a", choice: "rename", name: "Talk (2)" },
    { key: "c", choice: "rename", name: "Talk (3)" },
  ]);
});

test("the batch as it would run: skipped out, replaced marked, renamed renamed", () => {
  const items = [
    link("a", "Talk", "abc"),
    file("b", "/x/Lecture 1.mp4"),
    file("c", "/y/Lecture 1.mp4"),
    file("e", "/z/Lecture 1.mp4"),
    link("d", "Other", "def"),
  ];
  const run = asRun(items, [
    { key: "a", choice: "rename", name: " Talk (2) " },
    { key: "b", choice: "replace", name: "Lecture 1 (2)" },
    { key: "c", choice: "skip", name: "Lecture 1 (3)" },
    { key: "e", choice: "rename", name: "Lecture 1 (4)" },
  ]);
  assert.deepEqual(run.map((item) => item.id), ["a", "b", "e", "d"]);
  assert.equal(run[0].label, "Talk (2)");
  assert.equal(run[0].named, true);
  assert.equal(run[1].replace, true);
  assert.equal(run[1].name, undefined);
  assert.equal(run[2].name, "Lecture 1 (4)");
  assert.equal(run[2].label, "Lecture 1.mp4");
  assert.equal(run[3], items[4]);
});

test("a New name the library has, or that cannot be a folder, is refused", () => {
  const rows: NameRow[] = [
    { key: "a", choice: "rename", name: "Talk" },
    { key: "b", choice: "rename", name: ".x" },
  ];
  const found = refused(rows, [clash("a", { inLibrary: true }), clash("b", { usable: false })], null);
  assert.deepEqual([...found].sort(), ["a", "b"]);
});

test("Replace is never refused for the folder it replaces", () => {
  const rows: NameRow[] = [{ key: "a", choice: "replace", name: "Talk (2)" }];
  assert.equal(refused(rows, [clash("a", { inLibrary: true })], null).size, 0);
});

test("a clash with a job that has no row is pinned on the row, above or below", () => {
  const rows: NameRow[] = [{ key: "row", choice: "rename", name: "Lecture 1 (2)" }];
  assert.deepEqual([...refused(rows, [clash("other"), clash("row", { sameAs: "other" })], "row")], ["row"]);
  assert.deepEqual([...refused(rows, [clash("row"), clash("other", { sameAs: "row" })], "row")], ["row"]);
});

test("between two rows, the one just edited is refused, or else the later one", () => {
  const rows: NameRow[] = [
    { key: "a", choice: "rename", name: "X" },
    { key: "b", choice: "rename", name: "X" },
  ];
  const found = [clash("a"), clash("b", { sameAs: "a" })];
  assert.deepEqual([...refused(rows, found, "a")], ["a"]);
  assert.deepEqual([...refused(rows, found, null)], ["b"]);
});

test("what Replace takes away, from what is there", () => {
  assert.equal(
    replaceLine({ ...none, audio: true, transcript: true, screens: true, notes: true }),
    "MP3, transcript and screen text go · Notes stay",
  );
  assert.equal(replaceLine({ ...none, audio: true }), "MP3 goes");
  assert.equal(replaceLine({ ...none, audio: true, video: true }), "MP3 and video go");
});

test("a link says when it was finished, a file where it comes from", () => {
  const holds = { ...none, audio: true, transcript: true, notes: true, modified: Date.UTC(2026, 9, 24, 12) / 1000 };
  assert.equal(
    stateLine(link("a", "Talk", "abc"), clash("a", { inLibrary: true, holds })),
    "Finished 24 Oct · MP3 · Transcript · Notes",
  );
  assert.equal(
    stateLine(file("b", "/Users/someone/Downloads/Week 1/Lecture 1.mp4"), clash("b", { inLibrary: true, holds: { ...none, audio: true } })),
    "From ~/Downloads/Week 1 · MP3",
  );
  assert.equal(
    stateLine(file("c", "/Users/someone/Downloads/Week 2/Lecture 1.mp4"), clash("c", { sameAs: "b" })),
    "From ~/Downloads/Week 2 · Same name as another in this start",
  );
  assert.equal(fromFolder("/Volumes/Disk/Lecture.mp4"), "/Volumes/Disk");
});

test("a refused row says why", () => {
  assert.equal(refusalLine({ key: "a", choice: "rename", name: "  " }, clash("a", { usable: false })), "Needs a name");
  assert.equal(refusalLine({ key: "a", choice: "rename", name: ".x" }, clash("a", { usable: false })), "That name cannot be a folder");
  assert.equal(refusalLine({ key: "a", choice: "rename", name: "Talk" }, clash("a", { inLibrary: true })), "That name is taken too");
});
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `node --test src/lib/naming.test.ts`
Expected: FAIL, `Cannot find module '.../src/lib/naming.ts'`.

- [ ] **Step 3: Write the implementation**

Create `src/lib/naming.ts`:

```ts
/**
 * The dialog that asks about names the library already has, as plain
 * functions: which jobs it asks about, the batch as it would run with the
 * answers so far, which rows cannot start, and what each row's line says.
 *
 * Types are all it imports, so Node runs the tests for this file as is.
 */
import type { Candidate, Clash, Holds } from "./api";
import type { QueueItem } from "./queue";

export type Choice = "rename" | "replace" | "skip";

export type NameRow = {
  key: string;
  choice: Choice;
  /** What the New name field holds, suggested or typed. */
  name: string;
};

/** A job as the library check sees it. A link is checked only with its id. */
export function toCandidates(items: QueueItem[]): Candidate[] {
  return items.map((item) =>
    item.kind === "file"
      ? { key: item.id, path: item.target, name: item.name ?? null }
      : { key: item.id, title: item.named ? item.label : null, videoId: item.videoId ?? null },
  );
}

/** The rows: every job the first check found taken, with New name chosen. */
export function rowsFrom(clashes: Clash[]): NameRow[] {
  return clashes
    .filter((clash) => clash.inLibrary || clash.sameAs !== null)
    .map((clash) => ({ key: clash.key, choice: "rename", name: clash.suggestion ?? "" }));
}

/**
 * The batch as it would run with the answers so far: what the dialog checks
 * again on every change, and what it hands to the queue at the end.
 */
export function asRun(items: QueueItem[], rows: NameRow[]): QueueItem[] {
  const byKey = new Map(rows.map((row) => [row.key, row]));
  return items.flatMap((item): QueueItem[] => {
    const row = byKey.get(item.id);
    if (!row) return [item];
    if (row.choice === "skip") return [];
    if (row.choice === "replace") return [{ ...item, replace: true }];
    const name = row.name.trim();
    // A link's new name travels the way a pasted name does: as its title.
    return [item.kind === "file" ? { ...item, name } : { ...item, label: name, named: true }];
  });
}

/**
 * Which rows cannot start as they are. Only a row's name can change between
 * checks, so a clash is always a row's doing and is pinned on one: on the
 * row itself when the other job has none, and between two rows on the one
 * just edited, or else the later one. A Replace row is never refused for
 * the folder it means to replace.
 */
export function refused(rows: NameRow[], clashes: Clash[], edited: string | null): Set<string> {
  const choice = new Map(rows.map((row) => [row.key, row.choice]));
  const isRow = (key: string) => choice.has(key) && choice.get(key) !== "skip";
  const out = new Set<string>();
  for (const clash of clashes) {
    const row = isRow(clash.key);
    if (row && choice.get(clash.key) === "rename" && (!clash.usable || clash.inLibrary)) {
      out.add(clash.key);
    }
    const other = clash.sameAs;
    if (other === null) continue;
    if (row && isRow(other)) out.add(edited === other ? other : clash.key);
    else if (row) out.add(clash.key);
    else if (isRow(other)) out.add(other);
  }
  return out;
}

/** Why a refused row cannot start. */
export function refusalLine(row: NameRow, clash: Clash | undefined): string {
  if (row.name.trim() === "") return "Needs a name";
  if (clash && !clash.usable) return "That name cannot be a folder";
  return "That name is taken too";
}

function listed(parts: string[]): string {
  if (parts.length < 2) return parts[0] ?? "";
  return `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}`;
}

/** What Replace takes away, built from what the folder holds. */
export function replaceLine(holds: Holds): string {
  const goes = [
    holds.audio && "MP3",
    holds.transcript && "transcript",
    holds.video && "video",
    holds.screens && "screen text",
  ].filter((part): part is string => typeof part === "string");
  const line = goes.length === 0 ? "Nothing goes" : `${listed(goes)} ${goes.length === 1 ? "goes" : "go"}`;
  return holds.notes ? `${line} · Notes stay` : line;
}

/** "~/Downloads/Week 1": the folder a file comes from, home shortened. */
export function fromFolder(path: string): string {
  const parent = path.slice(0, path.lastIndexOf("/")) || "/";
  return parent.replace(/^\/Users\/[^/]+/, "~");
}

const DAY = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" });

/**
 * What a row says when nothing is wrong with it. A file says where it comes
 * from, since two rows can carry one file name; a link says when its folder
 * was finished. Then what the folder holds, or that another job has it.
 */
export function stateLine(item: QueueItem, clash: Clash): string {
  const parts: string[] = [];
  if (item.kind === "file") parts.push(`From ${fromFolder(item.target)}`);
  else if (clash.inLibrary) parts.push(`Finished ${DAY.format(new Date(clash.holds.modified * 1000))}`);
  if (clash.sameAs !== null) {
    parts.push("Same name as another in this start");
  } else {
    if (clash.holds.audio) parts.push("MP3");
    if (clash.holds.transcript) parts.push("Transcript");
    if (clash.holds.video) parts.push("Video");
    if (clash.holds.notes) parts.push("Notes");
  }
  return parts.join(" · ");
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `node --test src/lib/naming.test.ts`
Expected: 10 tests pass.

- [ ] **Step 5: Typecheck**

Run: `npm run build:vite`
Expected: success.

- [ ] **Step 6: Commit**

```bash
git add src/lib/naming.ts src/lib/naming.test.ts
git commit -m "The names dialog's logic, as plain functions with tests"
```

---

### Task 7: A segment that can be disabled, and the row's styles

**Files:**
- Modify: `src/components/Controls.tsx:3-26`
- Modify: `src/styles.css` (after `.segmented button[aria-checked="true"]`, and after `.prompt__actions`)

- [ ] **Step 1: Let `Segmented` disable an option**

In `src/components/Controls.tsx`, change the options type in `SegmentedProps` to:

```ts
  options: { value: T; label: string; disabled?: boolean }[];
```

and add `disabled={option.disabled}` to the `<button>` in `Segmented`, after `aria-checked={value === option.value}`.

- [ ] **Step 2: Style the disabled segment**

In `src/styles.css`, after the `.segmented button[aria-checked="true"]` rule, add:

```css
.segmented button:disabled {
  opacity: 0.35;
  cursor: default;
}
```

- [ ] **Step 3: Style the names dialog's rows**

In `src/styles.css`, after the `.prompt__actions` rule, add:

```css
/*
  The names dialog. A row that has chosen New name grows its field beneath
  it: the link field, one size down.
*/
.prompt__row--naming {
  grid-template-areas:
    "name choice"
    "state choice"
    "field field";
}

.prompt__field {
  grid-area: field;
  min-height: 2.25rem;
  margin-top: 0.375rem;
  padding: 0 0.75rem;
}

.prompt__field input {
  height: 2.25rem;
  min-width: 0;
  font-size: 0.875rem;
}

/* A name that cannot be used: its field and its line in the accent. */
.prompt__field[data-refused] {
  border-color: var(--accent);
}

.prompt__state[data-refused] {
  color: var(--accent);
}
```

- [ ] **Step 4: Typecheck**

Run: `npm run build:vite`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add src/components/Controls.tsx src/styles.css
git commit -m "A segment that can be disabled, and the names dialog's row"
```

---

### Task 8: The dialog

**Files:**
- Create: `src/components/NamePrompt.tsx`

- [ ] **Step 1: Write the component**

Create `src/components/NamePrompt.tsx`:

```tsx
import { useEffect, useRef, useState } from "react";
import { libraryClashes, type Clash } from "../lib/api";
import { rowEnter, sheetEnter } from "../lib/motion";
import {
  asRun,
  refusalLine,
  refused as refusedRows,
  replaceLine,
  rowsFrom,
  stateLine,
  toCandidates,
  type Choice,
  type NameRow,
} from "../lib/naming";
import type { QueueItem } from "../lib/queue";
import { Segmented } from "./Controls";

type Props = {
  /** The batch about to join the queue, in order. */
  items: QueueItem[];
  /** The first check's answer for every item. */
  clashes: Clash[];
  outDir: string | null;
  onConfirm: (items: QueueItem[]) => void;
  onCancel: () => void;
};

/**
 * What to do about jobs whose folder the library already has: a new name,
 * replace what is there (all but the notes), or skip. Asked once, before the
 * queue starts, so a run never stops halfway to ask.
 *
 * The rows are fixed when the dialog opens. Every change checks the batch
 * again as it would run, and that only marks fields: a name the library or
 * another job already uses keeps Start waiting.
 */
export function NamePrompt({ items, clashes, outDir, onConfirm, onCancel }: Props) {
  const [rows, setRows] = useState<NameRow[]>(() => rowsFrom(clashes));
  const [latest, setLatest] = useState<Map<string, Clash>>(
    () => new Map(clashes.map((clash) => [clash.key, clash])),
  );
  const [refused, setRefused] = useState<Set<string>>(new Set());
  /** A check is under way; Start waits for its answer. */
  const [checking, setChecking] = useState(true);
  const edited = useRef<string | null>(null);
  const sheet = useRef<HTMLDivElement>(null);
  const first = new Map(clashes.map((clash) => [clash.key, clash]));
  const ready = !checking && refused.size === 0;

  // Every change is checked again, a moment after typing stops.
  useEffect(() => {
    setChecking(true);
    let current = true;
    const timer = window.setTimeout(() => {
      libraryClashes(outDir, toCandidates(asRun(items, rows)))
        .then((found) => {
          if (!current) return;
          setLatest(new Map(found.map((clash) => [clash.key, clash])));
          setRefused(refusedRows(rows, found, edited.current));
        })
        // A check that cannot run must not hold the work back.
        .catch(() => current && setRefused(new Set()))
        .finally(() => current && setChecking(false));
    }, 150);
    return () => {
      current = false;
      window.clearTimeout(timer);
    };
  }, [rows, items, outDir]);

  useEffect(() => {
    if (!sheet.current) return;
    sheetEnter(sheet.current);
    sheet.current.focus();
  }, []);

  function confirm() {
    if (ready) onConfirm(asRun(items, rows));
  }

  // Escape cancels; Enter starts, from the field too. A focused button
  // answers Enter itself, so it is left alone.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
      if (event.key !== "Enter" || event.isComposing) return;
      if (event.target instanceof HTMLButtonElement) return;
      confirm();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function change(key: string, patch: Partial<NameRow>) {
    edited.current = key;
    setRows((prev) => prev.map((row) => (row.key === key ? { ...row, ...patch } : row)));
  }

  return (
    <div className="prompt">
      <div
        ref={sheet}
        className="prompt__sheet"
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-labelledby="names-title"
      >
        <h2 id="names-title" className="prompt__title">
          Already in the library
        </h2>
        <ul className="prompt__rows">
          {rows.map((row, index) => {
            const item = items.find((i) => i.id === row.key);
            const clash = first.get(row.key);
            if (!item || !clash) return null;
            return (
              <NameRowView
                key={row.key}
                row={row}
                index={index}
                item={item}
                first={clash}
                latest={latest.get(row.key)}
                refused={refused.has(row.key)}
                onChange={(patch) => change(row.key, patch)}
              />
            );
          })}
        </ul>
        <div className="prompt__actions">
          <button type="button" className="ghost ghost--small" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="cta cta--small" disabled={!ready} onClick={confirm}>
            Start
          </button>
        </div>
      </div>
    </div>
  );
}

function NameRowView({
  row,
  index,
  item,
  first,
  latest,
  refused,
  onChange,
}: {
  row: NameRow;
  index: number;
  item: QueueItem;
  /** What the folder held when the dialog opened; the row is about that. */
  first: Clash;
  /** The last check's answer, for why a typed name is refused. */
  latest: Clash | undefined;
  refused: boolean;
  onChange: (patch: Partial<NameRow>) => void;
}) {
  const ref = useRef<HTMLLIElement>(null);

  // Once, on arrival, like the language prompt's rows.
  useEffect(() => {
    if (ref.current) rowEnter(ref.current, index);
  }, []);

  const line = refused
    ? refusalLine(row, latest)
    : row.choice === "replace"
      ? replaceLine(first.holds)
      : row.choice === "skip"
        ? "Not run"
        : stateLine(item, first);

  return (
    // Only a New name row grows the third line, for its field.
    <li ref={ref} className={row.choice === "rename" ? "prompt__row prompt__row--naming" : "prompt__row"}>
      <span className="prompt__name" data-part>
        {item.label}
      </span>
      <span className="prompt__state" data-part data-refused={refused || undefined}>
        {line}
      </span>
      <span className="prompt__choice" data-part>
        <Segmented<Choice>
          label={`What to do with ${item.label}`}
          value={row.choice}
          onChange={(choice) => onChange({ choice })}
          options={[
            { value: "rename", label: "New name" },
            { value: "replace", label: "Replace", disabled: !first.replaceable },
            { value: "skip", label: "Skip" },
          ]}
        />
      </span>
      {row.choice === "rename" && (
        <label className="field prompt__field" data-refused={refused || undefined}>
          <input
            type="text"
            value={row.name}
            spellCheck={false}
            aria-label={`New name for ${item.label}`}
            onChange={(event) => onChange({ name: event.target.value })}
          />
        </label>
      )}
    </li>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `npm run build:vite`
Expected: success. (The component is not mounted yet; Task 9 does that.)

- [ ] **Step 3: Commit**

```bash
git add src/components/NamePrompt.tsx
git commit -m "The dialog that asks about a name the library already has"
```

---

### Task 9: One way into the queue

**Files:**
- Modify: `src/components/ConvertView.tsx` (imports, `Props`, the destructuring, `start`, the Convert button)
- Modify: `src/App.tsx` (imports, `readTitle`, `unpack`, the admission, `ConvertView` props, `LanguagePrompt`, the new dialog)

- [ ] **Step 1: ConvertView hands its batch over**

In `src/components/ConvertView.tsx`, change the queue import to:

```ts
import { fileItem, urlItem, type Pasted, type PendingLink, type QueueItem } from "../lib/queue";
```

In `Props`, after `onAddFiles: (paths: string[]) => void;`, add:

```ts
  /** Start: the batch goes to the library check, with the pills it came from. */
  onStart: (items: QueueItem[], pills: string[]) => void;
  /** A batch from here is being checked or asked about; Start waits. */
  starting: boolean;
```

Add `onStart,` and `starting,` to the destructured props after `onAddFiles,`.

Replace `start` with:

```ts
  function start() {
    if (links.length === 0) return;
    const items = links.map((l) =>
      l.file
        ? fileItem(l.url)
        : { ...urlItem(l.url, l.title ?? l.url, l.title !== null), videoId: l.videoId },
    );
    // The pills stay until the jobs are admitted: Cancel in the names
    // dialog leaves them where they were.
    onStart(items, links.map((l) => l.id));
  }
```

On the Convert button, change `disabled={links.length === 0 || nothingKept || !!blocked}` to:

```tsx
              disabled={links.length === 0 || nothingKept || !!blocked || starting}
```

- [ ] **Step 2: Pills learn their video id**

In `src/App.tsx`, in `readTitle`, change

```ts
      setLinks((all) => all.map((l) => (l.id === link.id ? { ...l, title: meta.title } : l)));
```

to

```ts
      setLinks((all) =>
        all.map((l) => (l.id === link.id ? { ...l, title: meta.title, videoId: meta.id } : l)),
      );
```

In `unpack`, change `.map((item) => knownLink(item.url, item.title));` to:

```ts
          .map((item) => ({ ...knownLink(item.url, item.title), videoId: item.id }));
```

- [ ] **Step 3: The admission**

In `src/App.tsx`, change the first `./lib/api` import to:

```ts
import {
  checkDependencies,
  defaultWhisperModel,
  libraryClashes,
  setGlass,
  type Clash,
  type DependencyStatus,
} from "./lib/api";
```

add `type QueueItem,` to the `./lib/queue` import list, and add after the `LanguagePrompt` import:

```ts
import { NamePrompt } from "./components/NamePrompt";
import { toCandidates } from "./lib/naming";
```

Above `export default function App()`, add:

```ts
/** Jobs on their way into the queue, and the Convert pills they came from. */
type Batch = { items: QueueItem[]; pills: string[] };
```

Right after `const jobs = useJobs(settings);`, add:

```ts
  // One way into the queue: checked against the library first, and asked
  // about if anything clashes. One batch at a time -- a recording that stops
  // while the dialog is open waits for it to close.
  const [naming, setNaming] = useState<{ batch: Batch; clashes: Clash[] } | null>(null);
  /** Pills on their way in; Start waits rather than sending them twice. */
  const [starting, setStarting] = useState(false);
  const admitting = useRef<Batch[]>([]);
  const admittingBusy = useRef(false);
  const outDir = useRef(settings.outDir);
  outDir.current = settings.outDir;

  function admit(batch: Batch) {
    if (batch.items.length === 0) return;
    if (batch.pills.length > 0) setStarting(true);
    admitting.current.push(batch);
    void admitNext();
  }

  async function admitNext() {
    if (admittingBusy.current) return;
    const batch = admitting.current.shift();
    if (!batch) return;
    admittingBusy.current = true;

    let clashes: Clash[] = [];
    try {
      clashes = await libraryClashes(outDir.current || null, toCandidates(batch.items));
    } catch {
      // A check that cannot run must not stop the work: it runs as it did.
    }
    if (clashes.some((clash) => clash.inLibrary || clash.sameAs !== null)) {
      setNaming({ batch, clashes });
      return;
    }
    settle(batch, batch.items);
  }

  /** The batch is through, as `items`, or cancelled when that is null. */
  function settle(batch: Batch, items: QueueItem[] | null) {
    if (items !== null) {
      const pills = new Set(batch.pills);
      setLinks((prev) => prev.filter((l) => !pills.has(l.id)));
      if (items.length > 0) void jobs.enqueue(items);
    }
    if (batch.pills.length > 0) setStarting(false);
    setNaming(null);
    admittingBusy.current = false;
    void admitNext();
  }
```

- [ ] **Step 4: Route Convert and the language dialog through it**

In the `<ConvertView ... />` element, after `onAddFiles={addFiles}` add:

```tsx
            onStart={(items, pills) => admit({ items, pills })}
            starting={starting}
```

Change the `LanguagePrompt` block to wait for the names dialog and to admit rather than enqueue:

```tsx
      {asking.length > 0 && !naming && (
        <LanguagePrompt
          paths={asking}
          whisperModelPath={settings.whisperModelPath || null}
          // The dialog offers Serbian and English; a fallback must be one of them.
          fallback={settings.transcriptLangs[0] === "en" ? "en" : "sr"}
          onConfirm={(files) => {
            setAsking([]);
            admit({
              items: files.map((file) => fileItem(file.path, file.language, true)),
              pills: [],
            });
          }}
          onCancel={() => setAsking([])}
        />
      )}

      {naming && (
        <NamePrompt
          items={naming.batch.items}
          clashes={naming.clashes}
          outDir={settings.outDir || null}
          onConfirm={(items) => settle(naming.batch, items)}
          onCancel={() => settle(naming.batch, null)}
        />
      )}
```

- [ ] **Step 5: The changelog line**

This is the commit in which someone using the app first sees the change,
so the line goes in with it. In `CHANGELOG.md`, under `## Unreleased` →
`### New`, add:

```markdown
- When something you start is already in the library, the app asks whether to give it a new name, replace it, or skip it.
```

Run: `python3 scripts/changelog.py text Unreleased`
Expected: the New line printed above the two Fixed lines.

- [ ] **Step 6: Typecheck and run every test**

Run: `npm run build:vite && node --test src/lib/naming.test.ts src/lib/releaseNotes.test.ts && python3 scripts/test_changelog.py && (cd src-tauri && cargo test)`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/components/ConvertView.tsx src/App.tsx CHANGELOG.md
git commit -m "Check every batch against the library before it joins the queue"
```

---

### Task 10: CLAUDE.md, and the real thing

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/superpowers/specs/2026-10-30-name-taken-design.md` (status line)

- [ ] **Step 1: Count the tests**

Run: `cd src-tauri && cargo test 2>&1 | grep "test result"`
Note the number passed in the first line, for Step 2.

- [ ] **Step 2: CLAUDE.md**

In the services list in `CLAUDE.md`, after `` `mp3` an MP3 of a file on disk``, add:

```markdown
    · `names` what a job's folder is called, and whether the library
    already has it
```

In "Current status", change `copying a chapter out and keeping what you bring back as a note.` to:

```markdown
copying a chapter out and keeping what you bring back as a note, and
asking what to do when a job's name is already in the library.
```

In "Commands", change `` `cd src-tauri && cargo test` -- 105 tests, all pure logic`` to the number from Step 1, and after the `releaseNotes.test.ts` line add:

```markdown
- `node --test src/lib/naming.test.ts` -- what the names dialog asks and refuses
```

In the spec, change `**Status:** designed 2026-10-30, not built.` to `**Status:** built 2026-10-30.`

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md docs/superpowers/specs/2026-10-30-name-taken-design.md
git commit -m "CLAUDE.md: the names service, its tests, and the test count"
```

- [ ] **Step 4: Run it in the app**

Ask the owner before replacing `/Applications/Tzz App.app`. The app must be quit first. Then `npm run install:app`, open the app from the Dock, and check each line of the spec's "Done when":

1. Paste a link that is already in the library (for example `Kratak osvrt na 2024. [7yUPpjJmTAg]`), wait for its title, press Convert. The dialog shows one row with `FINISHED <date> · MP3 …`, and New name is filled with `… (2)`.
2. New name → Start: a second folder `… (2) [id]` appears beside the first.
3. Run it again, choose Replace: the folder's MP3 and transcript are made again, and its `*.notes.md` is still there. If the folder had notes, the Reader still shows them against the new transcript.
4. Run it again, choose Skip: nothing runs for it.
5. Drop two different files, both named `Lecture 1.mp4`, from two folders, and press Convert. The second is asked about with `SAME NAME AS ANOTHER IN THIS START`, and Start gives two folders.
6. Drop an MP3 from its own library folder: no dialog.
7. The link that failed on 2026-10-30 (`agnzHC00dQ4`) runs again without a dialog and finishes.
8. Type a name the library has into a New name field: the field and its line turn amber and Start waits. Escape leaves the pills on Convert.
