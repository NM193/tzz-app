//! Recording commands.
//!
//! Start and stop both go through the tray module so the window and the menu
//! bar icon share one code path and one set of events.

use std::path::PathBuf;

use tauri::AppHandle;

use crate::services::recorder::{self, AudioInput};
use crate::services::tray;

#[tauri::command]
pub fn list_audio_inputs(app: AppHandle) -> Result<Vec<AudioInput>, String> {
    let inputs = recorder::list_inputs()?;
    // Devices may have appeared since the menu was built.
    tray::refresh(&app);
    Ok(inputs)
}

#[tauri::command]
pub fn set_audio_input(app: AppHandle, index: u32) {
    tray::set_input(&app, index);
    tray::refresh(&app);
}

/// Start if idle, stop if recording. The result arrives as an event.
#[tauri::command]
pub fn toggle_recording(app: AppHandle) {
    tray::toggle(&app);
}

#[tauri::command]
pub fn save_recording(temp_path: String, destination: String) -> Result<String, String> {
    let saved = recorder::save(&PathBuf::from(temp_path), &PathBuf::from(destination))?;
    Ok(saved.to_string_lossy().into_owned())
}
