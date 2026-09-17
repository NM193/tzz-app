//! What is already in the library folder.
//!
//! One folder per video is the only structure there is, so the listing is a
//! read of the top level: which folders exist and which kinds of file each one
//! holds. Nothing is indexed and nothing is remembered between launches -- the
//! folder itself is the database.

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryEntry {
    pub name: String,
    pub path: String,
    pub audio_path: Option<String>,
    pub video_path: Option<String>,
    pub markdown_path: Option<String>,
    pub pdf_path: Option<String>,
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
        modified: modified_at(folder),
    };

    for file in std::fs::read_dir(folder).ok()?.filter_map(Result::ok) {
        let path = file.path();
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        let as_string = || Some(path.to_string_lossy().into_owned());
        match ext.as_deref() {
            Some(e) if AUDIO.contains(&e) => entry.audio_path = entry.audio_path.take().or_else(as_string),
            Some(e) if VIDEO.contains(&e) => entry.video_path = entry.video_path.take().or_else(as_string),
            Some("md") => entry.markdown_path = entry.markdown_path.take().or_else(as_string),
            Some("pdf") => entry.pdf_path = entry.pdf_path.take().or_else(as_string),
            _ => {}
        }
        entry.modified = entry.modified.max(modified_at(&path));
    }

    Some(entry)
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
