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
