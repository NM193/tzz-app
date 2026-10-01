//! Recording commands.
//!
//! Start and stop both go through the tray module so the window and the menu
//! bar icon share one code path and one set of events.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::services::recorder::{self, AudioInput};
use crate::services::tray;

#[tauri::command]
pub fn list_audio_inputs(app: AppHandle) -> Result<Vec<AudioInput>, String> {
    let inputs = recorder::list_inputs()?;
    // Devices may have appeared since the menu was built.
    tray::refresh(&app);
    Ok(inputs)
}

/// The input mixed into the first, or none. Chosen in the window; the menu
/// bar icon only ever picks the main one.
#[tauri::command]
pub fn set_second_input(app: AppHandle, index: Option<u32>) {
    if let Ok(mut slot) = app.state::<recorder::RecorderState>().second_input.lock() {
        *slot = index;
    }
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

/// Open the page where BlackHole is distributed.
///
/// The app cannot install it: it is an audio driver, it needs an
/// administrator, and its authors hand out the installer through a form on
/// their own site rather than as a file anyone can fetch. So this opens their
/// page and she does the rest. The address is here rather than in the window
/// so that nothing on screen can decide where the app sends you.
#[tauri::command]
pub fn open_blackhole_page(app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::open_url("https://existential.audio/blackhole/", None::<&str>)
        .map_err(|e| format!("Could not open the page: {e}"))?;
    let _ = app;
    Ok(())
}
