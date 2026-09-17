use crate::services::library::{self, LibraryEntry};

use super::media::resolve_out_dir;

/// Everything in the library folder, newest first.
#[tauri::command]
pub fn list_library(out_dir: Option<String>) -> Result<Vec<LibraryEntry>, String> {
    let root = resolve_out_dir(out_dir.as_deref())?;
    Ok(library::list(&root))
}
