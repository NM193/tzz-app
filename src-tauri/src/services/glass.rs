//! The frosted window.
//!
//! The window is transparent and macOS paints a blurred copy of whatever is
//! behind it, the way Finder's sidebar does. The frontend draws no opaque
//! background of its own, so the effect shows through everywhere.
//!
//! `UnderWindowBackground` follows the window's appearance: the frontend
//! switches between light and dark by setting the window theme, and the
//! material changes with it. Nothing here needs to know which one is active.

use tauri::{AppHandle, Manager};

pub fn apply(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
        // Failing here means a square, opaque window: ugly but working. Not
        // worth refusing to start over.
        let _ = apply_vibrancy(
            &window,
            NSVisualEffectMaterial::UnderWindowBackground,
            Some(NSVisualEffectState::Active),
            Some(18.0),
        );
    }
}
