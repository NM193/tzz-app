//! What is already in the library folder.
//!
//! One folder per video is the only structure there is, so the listing is a
//! read of the top level: which folders exist and which kinds of file each one
//! holds. Nothing is indexed and nothing is remembered between launches -- the
//! folder itself is the database.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use super::thumb;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub name: String,
    pub path: String,
    pub audio_path: Option<String>,
    pub video_path: Option<String>,
    pub markdown_path: Option<String>,
    pub pdf_path: Option<String>,
    /// Your own notes, which are markdown too but a different document.
    pub notes_path: Option<String>,
    /// `poster.jpg`, when this folder already has one.
    pub poster_path: Option<String>,
    /// Seconds since the epoch, for sorting newest first.
    pub modified: u64,
}

const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac"];
const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov"];

pub fn list(root: &Path) -> Vec<LibraryEntry> {
    let Ok(dir) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut entries: Vec<LibraryEntry> = dir
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| describe(&entry.path()))
        .collect();

    entries.sort_by(|a, b| b.modified.cmp(&a.modified));
    entries
}

fn describe(folder: &Path) -> Option<LibraryEntry> {
    let name = folder.file_name()?.to_string_lossy().into_owned();
    if name.starts_with('.') {
        return None;
    }

    let mut entry = LibraryEntry {
        name,
        path: folder.to_string_lossy().into_owned(),
        audio_path: None,
        video_path: None,
        markdown_path: None,
        pdf_path: None,
        notes_path: None,
        poster_path: thumb::existing(folder).map(|p| p.to_string_lossy().into_owned()),
        modified: modified_at(folder),
    };

    for file in std::fs::read_dir(folder).ok()?.filter_map(Result::ok) {
        let path = file.path();
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        let as_string = || Some(path.to_string_lossy().into_owned());
        match ext.as_deref() {
            Some(e) if AUDIO.contains(&e) => entry.audio_path = entry.audio_path.take().or_else(as_string),
            Some(e) if VIDEO.contains(&e) => entry.video_path = entry.video_path.take().or_else(as_string),
            // `name.notes.md` is markdown as well; it is the notes, not the
            // transcript, and opening one for the other would be a puzzle.
            Some("md") if is_notes(&path) => {
                entry.notes_path = entry.notes_path.take().or_else(as_string)
            }
            Some("md") => entry.markdown_path = entry.markdown_path.take().or_else(as_string),
            Some("pdf") => entry.pdf_path = entry.pdf_path.take().or_else(as_string),
            _ => {}
        }
        entry.modified = entry.modified.max(modified_at(&path));
    }

    Some(entry)
}

fn is_notes(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".notes.md"))
}

fn modified_at(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("tzz-library-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn reports_which_files_each_folder_holds() {
        let root = temp_root("kinds");
        let folder = root.join("Lecture [abc]");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("Lecture [abc].mp3"), b"").unwrap();
        std::fs::write(folder.join("Lecture [abc].transcript.md"), b"").unwrap();
        std::fs::write(folder.join("Lecture [abc].screens.json"), b"").unwrap();

        let entries = list(&root);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Lecture [abc]");
        assert!(entries[0].audio_path.is_some());
        assert!(entries[0].markdown_path.is_some());
        assert!(entries[0].video_path.is_none());
        assert!(entries[0].pdf_path.is_none());
    }

    #[test]
    fn the_notes_are_not_mistaken_for_the_transcript() {
        let root = temp_root("notes");
        let folder = root.join("Lecture [abc]");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("Lecture [abc].md"), b"").unwrap();
        std::fs::write(folder.join("Lecture [abc].notes.md"), b"").unwrap();

        let entry = &list(&root)[0];
        assert!(entry.markdown_path.as_ref().unwrap().ends_with("Lecture [abc].md"));
        assert!(entry.notes_path.as_ref().unwrap().ends_with(".notes.md"));
    }

    #[test]
    fn skips_loose_files_and_hidden_folders() {
        let root = temp_root("loose");
        std::fs::write(root.join("Combined (2 videos).md"), b"").unwrap();
        std::fs::create_dir(root.join(".DS_Store_folder")).unwrap();
        std::fs::create_dir(root.join("Real")).unwrap();

        let names: Vec<String> = list(&root).into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["Real".to_string()]);
    }

    #[test]
    fn a_missing_root_is_an_empty_library() {
        assert!(list(Path::new("/definitely/not/here")).is_empty());
    }
}
