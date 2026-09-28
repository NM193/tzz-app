use crate::services::document::{self, Document};
use crate::services::library::{self, LibraryEntry};
use crate::services::notes;
use crate::services::thumb;

use super::media::resolve_out_dir;

/// Everything in the library folder, newest first.
#[tauri::command]
pub fn list_library(out_dir: Option<String>) -> Result<Vec<LibraryEntry>, String> {
    let root = resolve_out_dir(out_dir.as_deref())?;
    Ok(library::list(&root))
}

/// The folder's picture, made now if it has none.
///
/// Separate from the listing on purpose: drawing a frame takes a moment, and
/// the library must appear at once. The screen asks for the missing ones
/// afterwards, one at a time.
#[tauri::command]
pub async fn ensure_thumbnail(path: String) -> Option<String> {
    let folder = std::path::PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        thumb::ensure(&folder).map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}

/// A transcript, parsed into something the reader can lay out.
///
/// Only markdown: the PDF is for printing and Finder opens it better than we
/// could. The size cap is there because this is read into memory whole, and a
/// transcript that large is not a transcript.
#[tauri::command]
pub async fn open_document(path: String) -> Result<Document, String> {
    let path = std::path::PathBuf::from(path);

    if path.extension().and_then(|e| e.to_str()) != Some("md") {
        return Err("That is not a transcript.".to_string());
    }

    const LIMIT: u64 = 16 * 1024 * 1024;
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > LIMIT {
        return Err("That document is too large to open here.".to_string());
    }

    let markdown = tauri::async_runtime::spawn_blocking(move || std::fs::read_to_string(&path))
        .await
        .map_err(|_| "Could not read that document.".to_string())?
        .map_err(|e| format!("Could not read that document: {e}"))?;

    Ok(document::parse(&markdown))
}

/// Where a transcript's notes live: `<transcript>.notes.md`.
fn notes_path(transcript: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(transcript);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!("{stem}.notes.md"))
}

/// Every note kept for this transcript, as chapter title to text.
#[tauri::command]
pub fn read_notes(transcript: String) -> Vec<(String, String)> {
    let path = notes_path(&transcript);
    let Ok(markdown) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    notes::parse(notes::body_of(&markdown))
        .into_iter()
        .map(|note| (note.chapter.to_string(), note.body.to_string()))
        .collect()
}

/// Put a note against a chapter. An empty body removes it.
///
/// The whole file is rewritten each time: these are a few kilobytes, and a
/// rewrite cannot leave a half-edited document behind.
#[tauri::command]
pub fn save_note(
    transcript: String,
    title: String,
    chapter: String,
    body: String,
) -> Result<Option<String>, String> {
    let path = notes_path(&transcript);
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = notes::upsert(notes::body_of(&current), &chapter, &body);

    if updated.trim().is_empty() {
        // The last note was cleared; leave no empty document behind.
        let _ = std::fs::remove_file(&path);
        return Ok(None);
    }

    std::fs::write(&path, notes::document(&title, &updated))
        .map_err(|e| format!("Could not save the note: {e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
