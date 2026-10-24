//! Recording an audio input to MP3.
//!
//! This is the only part of the app that lives between two commands: the
//! capture and ffmpeg keep running from Record to Stop, so their handles are
//! held in Tauri-managed state rather than dropped at the end of a call.
//!
//! WHY the app reads the input and ffmpeg only encodes it: ffmpeg's own
//! avfoundation input lost about one buffer in ten (capture.rs has the
//! numbers). The samples come down a pipe instead, as raw 32-bit float.
//!
//! WHY only audio inputs and not "what the speakers play": macOS ships no
//! loopback device, so CoreAudio lists the microphone and nothing else. A
//! virtual driver such as BlackHole simply appears in the same list, which is
//! why nothing here special-cases it.
//!
//! WHY two inputs can be recorded at once: a call is the machine's sound and
//! your own voice, which are two devices. macOS can be told to combine them
//! into an Aggregate Device by hand, but the app reads both and ffmpeg mixes
//! them, and nobody should have to visit Audio MIDI Setup to record a call.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::binaries;
use super::capture::{self, Capture, Format};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioInput {
    /// The CoreAudio device id, not a position in this list.
    pub index: u32,
    pub name: String,
}

pub struct Recording {
    child: Child,
    captures: Vec<Capture>,
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
    /// A second input mixed into the first, for "the call and me".
    pub second_input: Mutex<Option<u32>>,
    /// The output device to put back when recording ends.
    pub restore_output: Mutex<Option<u32>>,
}

pub fn is_recording(state: &RecorderState) -> bool {
    state.current.lock().map(|slot| slot.is_some()).unwrap_or(false)
}

/// Every input the Mac can record from.
pub fn list_inputs() -> Result<Vec<AudioInput>, String> {
    Ok(capture::inputs().into_iter().map(|(index, name)| AudioInput { index, name }).collect())
}

/// The ffmpeg filter for one input, or for two mixed together.
///
/// Mixing two devices is not just `amix`. They are different shapes -- a
/// loopback is stereo, a built-in microphone is mono -- so each is first
/// brought to the same one. They also run on two clocks, and samples down a
/// pipe carry no clock, only their count, so nothing here keeps them in step:
/// two devices drift apart by a fraction of a second over an hour, which in a
/// call nobody hears.
///
/// `normalize=0` keeps both at their own level -- the default halves each,
/// which turns a quiet voice into nothing. The limiter is what makes that
/// safe: two loud sources can sum past full scale, and a clipped hour cannot
/// be repaired afterwards. `ebur128` is last so the meter reads what is
/// actually being written; `framelog=info` is load-bearing, because `verbose`
/// logs below ffmpeg's default level and the meter sits dead.
fn filter_for(second: Option<u32>) -> String {
    const SHAPE: &str = "aformat=sample_rates=48000:channel_layouts=stereo";

    match second {
        Some(_) => format!(
            "[0:a]{SHAPE}[a0];[1:a]{SHAPE}[a1];\
             [a0][a1]amix=inputs=2:duration=longest:normalize=0,\
             alimiter=limit=0.97,ebur128=framelog=info[a]"
        ),
        None => "[0:a]ebur128=framelog=info[a]".to_string(),
    }
}

pub fn start(
    app: &AppHandle,
    state: &RecorderState,
    input_index: u32,
    second_index: Option<u32>,
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

    let devices: Vec<u32> = std::iter::once(input_index).chain(second_index).collect();
    let formats = devices
        .iter()
        .map(|&device| capture::format_of(device).ok_or_else(|| "That input is no longer there.".to_string()))
        .collect::<Result<Vec<_>, _>>()?;

    // A second input reaches ffmpeg on descriptor 3; the read end is moved
    // there in the child, between fork and exec.
    let second_pipe = match second_index {
        Some(_) => Some(std::io::pipe().map_err(|e| format!("Could not start recording: {e}"))?),
        None => None,
    };

    let mut command = Command::new(ffmpeg);
    command
        .args(input_args(&formats))
        .args(["-filter_complex", &filter_for(second_index)])
        .args(["-map", "[a]"])
        .args(["-c:a", "libmp3lame", "-q:a", "0", "-y"])
        .arg(&temp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if let Some((reader, _)) = &second_pipe {
        let fd = reader.as_raw_fd();
        unsafe { command.pre_exec(move || onto_descriptor_three(fd)) };
    }
    let mut child = command.spawn().map_err(|e| format!("Could not start recording: {e}"))?;

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

    // ffmpeg holds the ends it reads from now; the app keeps the ends it
    // writes to, one per input, and starts filling them.
    let mut outs: Vec<Box<dyn Write + Send>> = Vec::new();
    if let Some(stdin) = child.stdin.take() {
        outs.push(Box::new(stdin));
    }
    if let Some((reader, writer)) = second_pipe {
        drop(reader);
        outs.push(Box::new(writer));
    }

    let mut captures = Vec::new();
    for ((&device, &format), out) in devices.iter().zip(&formats).zip(outs) {
        match Capture::start(device, format, out) {
            Ok(capture) => captures.push(capture),
            Err(message) => {
                // Half a recording is not what was asked for.
                drop(captures);
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&temp_path);
                return Err(message);
            }
        }
    }

    // The pid outlives us on purpose: if this app is killed rather than closed,
    // the next launch uses it to stop the recorder we left behind.
    let _ = std::fs::write(pid_file(temp_dir), child.id().to_string());

    *slot = Some(Recording {
        child,
        captures,
        temp_path,
        started: Instant::now(),
        temp_dir: temp_dir.to_path_buf(),
    });
    Ok(())
}

/// What ffmpeg is told about the samples coming down each pipe: raw 32-bit
/// float at the device's own rate, the first input on stdin and a second on
/// descriptor 3.
///
/// Told everything, ffmpeg still reads about four seconds of each input to
/// "probe" it, one input after the other, before it starts. With one input
/// the meter sat dead that long; with two, the second pipe went unread for
/// four seconds and its queue overflowed. `-probesize 32 -analyzeduration 0`
/// skips that: the first reading comes after 0.17 s.
fn input_args(formats: &[Format]) -> Vec<String> {
    formats
        .iter()
        .zip(["pipe:0", "pipe:3"])
        .flat_map(|(format, pipe)| {
            [
                "-probesize".to_string(),
                "32".to_string(),
                "-analyzeduration".to_string(),
                "0".to_string(),
                "-f".to_string(),
                "f32le".to_string(),
                "-ar".to_string(),
                (format.rate.round() as u32).to_string(),
                "-ac".to_string(),
                format.channels.to_string(),
                "-i".to_string(),
                pipe.to_string(),
            ]
        })
        .collect()
}

/// Put the second input's pipe where `-i pipe:3` reads it. This runs in the
/// child between fork and exec, so it makes system calls and nothing else.
fn onto_descriptor_three(fd: RawFd) -> std::io::Result<()> {
    const F_SETFD: i32 = 2;
    // A pipe that already is descriptor 3 does not need moving, but it is
    // marked close-on-exec, and exec would close it: clear the mark instead.
    let status = if fd == 3 { unsafe { fcntl(3, F_SETFD, 0) } } else { unsafe { dup2(fd, 3) } };
    if status == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

extern "C" {
    fn dup2(from: i32, to: i32) -> i32;
    fn fcntl(fd: i32, command: i32, ...) -> i32;
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

    // Closing the pipes is the end of the input: ffmpeg flushes what it has,
    // finishes the MP3 and exits by itself. Killing it would leave a file with
    // no proper header, which nothing can play.
    for capture in recording.captures.drain(..) {
        capture.stop();
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
/// A killed app used to leave ffmpeg recording on its own. Now its pipes close
/// with the app, so ffmpeg finishes the file and exits by itself, and by the
/// next launch the pid it left may belong to something else entirely. So this
/// stops only a process that is still one of ours: ffmpeg, writing into the
/// recording folder. SIGINT rather than SIGKILL, so its file stays playable.
pub fn stop_orphan(temp_dir: &Path) {
    let path = pid_file(temp_dir);
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    let _ = std::fs::remove_file(&path);

    let Ok(pid) = contents.trim().parse::<u32>() else {
        return;
    };
    let Ok(listing) = Command::new("/bin/ps").args(["-o", "args=", "-p", &pid.to_string()]).output() else {
        return;
    };
    if !is_recorder(&String::from_utf8_lossy(&listing.stdout), temp_dir) {
        return;
    }

    let _ = Command::new("/bin/kill")
        .args(["-INT", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Whether a process's command line is one of this app's recorders.
fn is_recorder(command_line: &str, temp_dir: &Path) -> bool {
    command_line.contains("ffmpeg") && command_line.contains(&*temp_dir.to_string_lossy())
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

    #[test]
    fn one_input_arrives_on_stdin_as_raw_float() {
        let args = input_args(&[Format { rate: 48_000.0, channels: 2 }]);
        assert_eq!(
            args,
            [
                "-probesize", "32", "-analyzeduration", "0", "-f", "f32le", "-ar", "48000", "-ac", "2",
                "-i", "pipe:0"
            ]
        );
    }

    #[test]
    fn a_second_input_arrives_on_descriptor_three() {
        let args = input_args(&[
            Format { rate: 48_000.0, channels: 2 },
            Format { rate: 44_100.0, channels: 1 },
        ]);
        assert_eq!(
            &args[12..],
            [
                "-probesize", "32", "-analyzeduration", "0", "-f", "f32le", "-ar", "44100", "-ac", "1",
                "-i", "pipe:3"
            ]
        );
    }

    #[test]
    fn only_a_recorder_writing_into_our_folder_is_stopped() {
        let folder = Path::new("/var/folders/x/T/yt-mp3-recording");
        assert!(is_recorder(
            "/Applications/Tzz App.app/Contents/MacOS/ffmpeg -f f32le -ar 48000 -ac 2 -i pipe:0 \
             -y /var/folders/x/T/yt-mp3-recording/recording-1792834001.mp3",
            folder
        ));
        // The pid went to something else, or to nothing at all.
        assert!(!is_recorder("/usr/bin/vim notes.txt", folder));
        assert!(!is_recorder("", folder));
    }

    #[test]
    fn one_input_is_passed_through_and_two_are_mixed() {
        assert_eq!(filter_for(None), "[0:a]ebur128=framelog=info[a]");

        let both = filter_for(Some(1));
        assert!(both.contains("amix=inputs=2"), "{both}");
        // A stereo loopback and a mono microphone are different shapes.
        assert_eq!(both.matches("channel_layouts=stereo").count(), 2, "{both}");
        // Halving each is what the default does, and a quiet voice disappears.
        assert!(both.contains("normalize=0"), "{both}");
        // Two loud sources sum past full scale.
        assert!(both.contains("alimiter"), "{both}");
        assert!(both.ends_with("ebur128=framelog=info[a]"), "{both}");
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

}
