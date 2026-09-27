//! The frosted window, and turning it off again.
//!
//! macOS paints a blurred copy of whatever is behind the window, the way
//! Finder's sidebar does. It is a taste question rather than a right answer:
//! the blur eats the hairlines and the dimmed greys the rest of the design is
//! built on, so the app offers it as a setting and starts without it.
//!
//! The window is always transparent (tauri.conf.json). With the glass off,
//! the frontend simply paints an opaque background over it.

use tauri::{AppHandle, Manager};

pub fn apply(app: &AppHandle, glass: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    #[cfg(target_os = "macos")]
    {
        use window_vibrancy::{
            apply_vibrancy, clear_vibrancy, NSVisualEffectMaterial, NSVisualEffectState,
        };
        // Failing either way means a plain window: never a reason to refuse to
        // start, and never a reason to fail the command.
        if glass {
            let _ = apply_vibrancy(
                &window,
                NSVisualEffectMaterial::UnderWindowBackground,
                Some(NSVisualEffectState::Active),
                Some(18.0),
            );
        } else {
            let _ = clear_vibrancy(&window);
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, glass);
}
