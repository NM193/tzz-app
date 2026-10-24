//! The menu bar icon.
//!
//! Recording is started far more often than the window is opened -- a call
//! begins and you want one click -- so the icon toggles directly on a left
//! click and keeps the menu for everything else.
//!
//! Both the icon and the window go through `toggle`, so there is one code path
//! and the two can never disagree about what is happening.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use super::audio_output;
use super::recorder::{self, RecorderState};

const TRAY_ID: &str = "recorder";
const MENU_TOGGLE: &str = "toggle";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";
/// Menu ids for inputs are "input:<CoreAudio device id>".
const MENU_INPUT_PREFIX: &str = "input:";

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    // A previous session may have been killed mid-recording. Stop what it left
    // running, and put the sound back through the speakers -- an app that
    // crashed while routed through the Multi-Output Device would otherwise
    // leave the volume keys dead with nothing left to fix them.
    recorder::stop_orphan(&recording_temp_dir());
    restore_output(app);

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(idle_icon()?)
        // Template images are recoloured by macOS, so the icon stays legible
        // in a light menu bar and a dark one.
        .icon_as_template(true)
        .menu(&menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle(tray.app_handle());
            }
        })
        .build(app)?;

    // Keep the handle so the icon can change while recording.
    let _ = tray;
    Ok(())
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let recording = is_recording(app);
    let label = if recording { "Stop recording" } else { "Start recording" };

    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, MENU_TOGGLE, label, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    for input in recorder::list_inputs().unwrap_or_default() {
        let id = format!("{MENU_INPUT_PREFIX}{}", input.index);
        let chosen = selected_input(app) == Some(input.index);
        let label = if chosen { format!("• {}", input.name) } else { format!("   {}", input.name) };
        menu.append(&MenuItem::with_id(app, id, label, !recording, None::<&str>)?)?;
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, MENU_OPEN, "Open Tzz App", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?)?;

    Ok(menu)
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        MENU_TOGGLE => toggle(app),
        MENU_OPEN => show_window(app),
        MENU_QUIT => app.exit(0),
        other => {
            if let Some(index) = other.strip_prefix(MENU_INPUT_PREFIX).and_then(|i| i.parse().ok()) {
                set_input(app, index);
                let _ = app.emit("recording://input", index);
                refresh(app);
            }
        }
    }
}

/// Start or stop, and tell the window what happened.
///
/// The window never stops a recording by itself; it calls in here too, so the
/// naming dialog is driven by one event no matter which button was pressed.
pub fn toggle(app: &AppHandle) {
    let state = app.state::<RecorderState>();

    if recorder::is_recording(&state) {
        match recorder::stop(&state) {
            Ok(result) => {
                restore_output(app);
                let _ = app.emit("recording://stopped", result);
                show_window(app);
            }
            Err(message) => {
                let _ = app.emit("recording://error", message);
            }
        }
    } else {
        let index = selected_input(app).or_else(first_input).unwrap_or(0);
        let second = second_input(app);
        match_rate_for(app, index);
        match recorder::start(app, &state, index, second, &recording_temp_dir()) {
            Ok(()) => {
                route_output_for(app, index);
                let _ = app.emit("recording://started", index);
            }
            Err(message) => {
                let _ = app.emit("recording://error", message);
            }
        }
    }

    refresh(app);
}

/// Redraw the icon and rebuild the menu after anything changes.
pub fn refresh(app: &AppHandle) {
    let recording = is_recording(app);
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    let icon = if recording { recording_icon() } else { idle_icon() };
    if let Ok(icon) = icon {
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_icon_as_template(true);
    }
    if let Ok(menu) = menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// Send the Mac's sound through the Multi-Output Device while recording.
///
/// Only when the chosen input is a capture driver: recording the microphone has
/// nothing to do with where sound comes out, and switching would be rude.
/// The Multi-Output Device is left unselected the rest of the time because it
/// disables the volume keys.
fn route_output_for(app: &AppHandle, input_index: u32) {
    if !captures_system_audio(input_index) {
        return;
    }
    let Some((device, name)) = audio_output::find_multi_output() else {
        let _ = app.emit(
            "recording://error",
            "No Multi-Output Device found, so the Mac's sound is not being captured. \
             Create one in Audio MIDI Setup with your speakers and BlackHole.",
        );
        return;
    };

    let previous = audio_output::current_output();
    if previous == Some(device) {
        return; // Already routed; nothing to put back afterwards.
    }

    if audio_output::set_output(device).is_ok() {
        if let Ok(mut slot) = app.state::<RecorderState>().inner().restore_output.lock() {
            *slot = previous;
        }
        let _ = app.emit("recording://output", name);
    }
}

fn restore_output(app: &AppHandle) {
    let state = app.state::<RecorderState>().inner();
    let previous = state.restore_output.lock().ok().and_then(|mut slot| slot.take());

    if let Some(device) = previous {
        let _ = audio_output::set_output(device);
        return;
    }

    // Nothing remembered, but if the Mac is still playing through a
    // Multi-Output Device then this app is what put it there. Alert sounds stay
    // on real hardware, so that is what to go back to.
    let Some(current) = audio_output::current_output() else {
        return;
    };
    let sitting_on_multi = audio_output::name_of(current).is_some_and(|n| audio_output::is_multi_output(&n));
    if sitting_on_multi {
        if let Some(hardware) = audio_output::system_output() {
            let _ = audio_output::set_output(hardware);
        }
    }
}

/// Called on the way out.
///
/// Quitting mid-recording would otherwise leave the Mac routed through the
/// Multi-Output Device, with the volume keys dead and no app left to fix it.
/// The audio itself is finalised too, so the temp file stays playable.
pub fn shutdown(app: &AppHandle) {
    let state = app.state::<RecorderState>();
    if recorder::is_recording(&state) {
        let _ = recorder::stop(&state);
    }
    restore_output(app);
}

/// Run the capture driver at the Multi-Output Device's sample rate.
///
/// Before the recording starts, not after: a rate changed under a running
/// capture changes its format mid-stream. A Multi-Output Device does not do
/// this for the devices inside it, and with BlackHole at 44.1 kHz under one at
/// 48 kHz the recording came out 9% fast, twice as loud and garbled.
fn match_rate_for(app: &AppHandle, input_index: u32) {
    let Some(name) = input_name(input_index).filter(|name| is_capture_driver(name)) else {
        return;
    };
    // No Multi-Output Device is reported once the recording starts.
    let Some((multi, _)) = audio_output::find_multi_output() else {
        return;
    };
    let (Some(wanted), Some(current)) =
        (audio_output::nominal_rate(multi), audio_output::nominal_rate(input_index))
    else {
        return;
    };
    if (wanted - current).abs() < 0.5 {
        return;
    }

    if audio_output::set_nominal_rate(input_index, wanted).is_err() {
        let _ = app.emit(
            "recording://error",
            format!(
                "{name} runs at {} kHz and the Multi-Output Device at {} kHz, and the app could not \
                 change it. Set both to the same rate in Audio MIDI Setup.",
                current / 1000.0,
                wanted / 1000.0
            ),
        );
    }
}

fn captures_system_audio(input_index: u32) -> bool {
    input_name(input_index).is_some_and(|name| is_capture_driver(&name))
}

/// BlackHole and aggregate devices built on it are what carries system sound.
fn is_capture_driver(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("blackhole") || name.contains("aggregate")
}

/// What to record from when nothing has been chosen: the first input listed.
/// An input is a CoreAudio device id, and 0 is no device at all.
fn first_input() -> Option<u32> {
    recorder::list_inputs().ok()?.first().map(|input| input.index)
}

fn input_name(input_index: u32) -> Option<String> {
    recorder::list_inputs()
        .unwrap_or_default()
        .into_iter()
        .find(|input| input.index == input_index)
        .map(|input| input.name)
}

pub fn set_input(app: &AppHandle, index: u32) {
    let state = app.state::<RecorderState>().inner();
    if let Ok(mut selected) = state.selected_input.lock() {
        *selected = Some(index);
    }
}

/// The input mixed into the first, when the window has asked for one.
fn second_input(app: &AppHandle) -> Option<u32> {
    app.state::<RecorderState>().second_input.lock().ok().and_then(|slot| *slot)
}

fn selected_input(app: &AppHandle) -> Option<u32> {
    // `inner()` ties the borrow to the app rather than to the temporary State
    // handle, which an `if let` or `?` would otherwise outlive.
    let state = app.state::<RecorderState>().inner();
    let selected = state.selected_input.lock().ok()?;
    *selected
}

fn is_recording(app: &AppHandle) -> bool {
    recorder::is_recording(&app.state::<RecorderState>())
}

fn recording_temp_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("yt-mp3-recording")
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

// Baked into the binary: the resource directory differs between `tauri dev`
// and a bundled app, and an icon that fails to load leaves no menu bar item.
const IDLE_PNG: &[u8] = include_bytes!("../../icons/tray-idle.png");
const RECORDING_PNG: &[u8] = include_bytes!("../../icons/tray-recording.png");

fn idle_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(IDLE_PNG)
}

fn recording_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(RECORDING_PNG)
}
