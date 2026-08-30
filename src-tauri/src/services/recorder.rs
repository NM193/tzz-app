//! Recording an audio input to MP3.
//!
//! This is the only part of the app that lives between two commands: ffmpeg
//! keeps running from Record to Stop, so its handle is held in Tauri-managed
//! state rather than dropped at the end of a call.
//!
//! WHY only audio inputs and not "what the speakers play": macOS ships no
//! loopback device, so `avfoundation` lists the microphone and nothing else.
//! A virtual driver such as BlackHole simply appears in the same list, which
//! is why nothing here special-cases it.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::binaries;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInput {
    /// The index avfoundation expects, not a position in this list.
    pub index: u32,
    pub name: String,
}

pub struct Recording {
    child: Child,
    temp_path: PathBuf,
    started: Instant,
    temp_dir: PathBuf,
}

/// Tauri-managed. Holds at most one recording, and which input to use.
///
/// The chosen input lives here rather than in the window because the menu bar
/// icon can start a recording with no window open.
#[derive(Default)]
pub struct RecorderState {
    pub current: Mutex<Option<Recording>>,
    pub selected_input: Mutex<Option<u32>>,
    /// The output device to put back when recording ends.
    pub restore_output: Mutex<Option<u32>>,
}

pub fn is_recording(state: &RecorderState) -> bool {
    state.current.lock().map(|slot| slot.is_some()).unwrap_or(false)
}

/// Ask ffmpeg what it can record from.
pub fn list_inputs() -> Result<Vec<AudioInput>, String> {
    let ffmpeg = binaries::require("ffmpeg")?;

    // Listing devices is an "error" as far as ffmpeg is concerned: it prints
    // the list, then exits complaining there is no input to process.
    let output = Command::new(ffmpeg)
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .output()
        .map_err(|e| format!("Could not ask ffmpeg for the audio inputs: {e}"))?;

    Ok(parse_inputs(&String::from_utf8_lossy(&output.stderr)))
}

/// Pull the audio devices out of ffmpeg's listing.
///
/// Video devices are printed first in the same `[n] Name` shape, so the audio
/// heading is what tells the two apart.
fn parse_inputs(listing: &str) -> Vec<AudioInput> {
    let mut inputs = Vec::new();
    let mut in_audio_section = false;

    for line in listing.lines() {
        let line = match line.split_once("] ") {
            // Strip the "[AVFoundation indev @ 0x...]" prefix.
            Some((prefix, rest)) if prefix.contains("AVFoundation") => rest,
            _ => continue,
        };

        if line.starts_with("AVFoundation audio devices:") {
            in_audio_section = true;
            continue;
        }
        if line.starts_with("AVFoundation video devices:") {
            in_audio_section = false;
            continue;
        }
        if !in_audio_section {
            continue;
        }

        if let Some((index, name)) = line.strip_prefix('[').and_then(|l| l.split_once("] ")) {
            if let Ok(index) = index.parse::<u32>() {
                inputs.push(AudioInput { index, name: name.trim().to_string() });
            }
        }
    }

    inputs
}

pub fn start(
    app: &AppHandle,
    state: &RecorderState,
    input_index: u32,
    temp_dir: &Path,
) -> Result<(), String> {
    let mut slot = state.current.lock().map_err(|_| lock_error())?;
    if slot.is_some() {
        return Err("A recording is already running.".to_string());
    }

    let ffmpeg = binaries::require("ffmpeg")?;
    std::fs::create_dir_all(temp_dir)
        .map_err(|e| format!("Could not create the recording folder: {e}"))?;
    // A name per recording, so a straggler finalising an old file can never be
    // overwritten by the next one.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let temp_path = temp_dir.join(format!("recording-{stamp}.mp3"));

    // stdin stays open: stopping means sending ffmpeg a "q", not killing it.
    // ebur128 passes the audio through untouched and prints a loudness reading
    // roughly every 100 ms, which is what the meter draws. `framelog=info` is
    // load-bearing: `verbose` logs below ffmpeg's default level, so the
    // readings never appear and the meter sits dead.
    let mut child = Command::new(ffmpeg)
        .args(["-f", "avfoundation", "-i", &format!(":{input_index}")])
        .args(["-af", "ebur128=framelog=info"])
        .args(["-c:a", "libmp3lame", "-q:a", "0", "-y"])
        .arg(&temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start recording: {e}"))?;

    if let Some(stderr) = child.stderr.take() {
        let app = app.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if let Some(level) = parse_level(&line) {
                    // A dropped meter reading is nothing; never abort on it.
                    let _ = app.emit("recording://level", level);
                }
            }
        });
    }

    // The pid outlives us on purpose: if this app is killed rather than closed,
    // the next launch uses it to stop the recorder we left behind.
    let _ = std::fs::write(pid_file(temp_dir), child.id().to_string());

    *slot = Some(Recording { child, temp_path, started: Instant::now(), temp_dir: temp_dir.to_path_buf() });
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResult {
    /// Where the finished MP3 sits until it is given a name.
    pub temp_path: String,
    pub seconds: u64,
}

/// Read one loudness value out of ffmpeg's ebur128 output.
///
/// The line looks like `t: 1.3  M: -25.5 S: -27.1  I: -26.5 LUFS  LRA: 1.4 LU`.
/// `M` is momentary loudness in LUFS: about -60 is silence and 0 is as loud as
/// it gets, so the meter maps that range onto 0..1.
fn parse_level(line: &str) -> Option<f32> {
    let after = line.split(" M:").nth(1)?;
    let lufs: f32 = after.split_whitespace().next()?.parse().ok()?;
    Some(((lufs + 60.0) / 60.0).clamp(0.0, 1.0))
}

/// Stop gracefully so the MP3 is playable.
pub fn stop(state: &RecorderState) -> Result<RecordingResult, String> {
    let mut slot = state.current.lock().map_err(|_| lock_error())?;
    let Some(mut recording) = slot.take() else {
        return Err("Nothing is being recorded.".to_string());
    };

    let seconds = recording.started.elapsed().as_secs();

    // "q" is ffmpeg's own quit key. Killing it here would leave a file with no
    // proper header, which nothing can play.
    if let Some(stdin) = recording.child.stdin.as_mut() {
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }

    recording
        .child
        .wait()
        .map_err(|e| format!("The recorder did not stop cleanly: {e}"))?;

    let _ = std::fs::remove_file(pid_file(&recording.temp_dir));

    if !recording.temp_path.is_file() {
        return Err("The recording produced no audio.".to_string());
    }

    Ok(RecordingResult { temp_path: recording.temp_path.to_string_lossy().into_owned(), seconds })
}

fn pid_file(temp_dir: &Path) -> PathBuf {
    temp_dir.join("recording.pid")
}

/// Stop a recorder left running by a previous, ungracefully ended session.
///
/// A killed app takes nothing with it: ffmpeg keeps recording, and once the
/// earlier file has been saved it keeps writing into that saved file. SIGINT
/// rather than SIGKILL, so whatever it was writing stays playable.
pub fn stop_orphan(temp_dir: &Path) {
    let path = pid_file(temp_dir);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    let _ = std::fs::remove_file(&path);

    let Ok(pid) = contents.trim().parse::<u32>() else {
        return;
    };

    let _ = Command::new("/bin/kill")
        .args(["-INT", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Move the finished recording where the user chose.
pub fn save(temp_path: &Path, destination: &Path) -> Result<PathBuf, String> {
    if !temp_path.is_file() {
        return Err("That recording is no longer there.".to_string());
    }

    // Rename fails across volumes; copying covers an external disk.
    if std::fs::rename(temp_path, destination).is_err() {
        std::fs::copy(temp_path, destination)
            .map_err(|e| format!("Could not save the recording: {e}"))?;
        let _ = std::fs::remove_file(temp_path);
    }

    Ok(destination.to_path_buf())
}

fn lock_error() -> String {
    "The recorder is in a bad state. Restart the app.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "\
[AVFoundation indev @ 0x1] AVFoundation video devices:
[AVFoundation indev @ 0x1] [0] MacBook Pro Camera
[AVFoundation indev @ 0x1] [1] Capture screen 0
[AVFoundation indev @ 0x1] AVFoundation audio devices:
[AVFoundation indev @ 0x1] [0] MacBook Pro Microphone
[AVFoundation indev @ 0x1] [1] BlackHole 2ch
";

    #[test]
    fn takes_the_audio_devices_and_not_the_cameras() {
        let inputs = parse_inputs(LISTING);
        assert_eq!(
            inputs,
            vec![
                AudioInput { index: 0, name: "MacBook Pro Microphone".into() },
                AudioInput { index: 1, name: "BlackHole 2ch".into() },
            ]
        );
    }

    #[test]
    fn copes_with_a_machine_that_reports_no_audio_inputs() {
        let listing = "[AVFoundation indev @ 0x1] AVFoundation video devices:\n\
                       [AVFoundation indev @ 0x1] [0] MacBook Pro Camera\n";
        assert!(parse_inputs(listing).is_empty());
    }

    // Copied from ffmpeg's actual output: no space after "M:".
    #[test]
    fn reads_loudness_out_of_a_real_ebur128_line() {
        let line = "[Parsed_ebur128_0 @ 0x866c08f00] t: 0.199938   TARGET:-23 LUFS    \
                    M:-30.0 S:-27.1     I: -26.5 LUFS       LRA:   1.4 LU";
        let level = parse_level(line).unwrap();
        assert!((level - 0.5).abs() < 0.01, "half loudness, got {level}");
    }

    #[test]
    fn silence_reads_as_an_empty_meter() {
        let line = "[Parsed_ebur128_0 @ 0x1] t: 0.0999375  TARGET:-23 LUFS    \
                    M:-120.7 S:-120.7     I: -70.0 LUFS       LRA:   0.0 LU";
        assert_eq!(parse_level(line), Some(0.0));
    }

    #[test]
    fn ignores_lines_without_a_reading() {
        assert_eq!(parse_level("ffmpeg version 7.1"), None);
        assert_eq!(parse_level("[Parsed_ebur128_0 @ 0x1] Summary:"), None);
    }

    #[test]
    fn ignores_unrelated_ffmpeg_chatter() {
        assert!(parse_inputs("ffmpeg version 7.1\nbuilt with clang\n").is_empty());
    }
}
