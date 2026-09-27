use tauri::AppHandle;

use crate::services::glass;

/// Turn the frosted window on or off while the app is running.
#[tauri::command]
pub fn set_glass(app: AppHandle, glass: bool) {
    glass::apply(&app, glass);
}
