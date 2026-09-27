use crate::services::document::{self, Document};
use crate::services::library::{self, LibraryEntry};
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
