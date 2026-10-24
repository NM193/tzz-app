//! Which language a file is spoken in, asked of Whisper before transcribing.
//!
//! Whisper's guess is only ever a default. It decides from the first 30
//! seconds it is given, and on 2026-10-24 Serbian over music came back as
//! English, so the dialog shows the guess and the owner confirms or corrects
//! it. This module hears; naming and grouping languages is the dialog's job.
//!
//! Deliberately not registered with `cancel`: Stop on a running job must not
//! reach a detection, nor a detection be mistaken for the job.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tokio::process::Command;

use super::binaries;

/// All Whisper listens to when it guesses, so all there is to cut.
const EXCERPT_SECONDS: f64 = 30.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    /// Whisper's code: "sr", "hr", "en", ...
    pub code: String,
    /// 0.0-1.0, how sure it was.
    pub probability: f32,
}

/// `Duration: 00:12:34.56` from ffmpeg's description of an input. `None` for
/// `Duration: N/A` -- a stream that does not know its length -- or no line.
pub fn parse_duration(header: &str) -> Option<f64> {
    let rest = header.split("Duration: ").nth(1)?;
    let clock = rest.split(',').next()?.trim();
    let mut parts = clock.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Where the excerpt starts: a third of the way in, so a lecture's intro
/// music does not decide, but never so late that less than 30 s is left.
/// A short file, or one whose length is unknown, is heard from the start.
pub fn excerpt_start(duration: Option<f64>) -> f64 {
    match duration {
        Some(total) if total > EXCERPT_SECONDS => (total / 3.0).min(total - EXCERPT_SECONDS),
        _ => 0.0,
    }
}

/// `auto-detected language: hr (p = 0.526195)` from whisper-cli's log.
pub fn parse_detected(line: &str) -> Option<Detected> {
    let rest = line.split("auto-detected language: ").nth(1)?;
    let (code, rest) = rest.split_once(" (p = ")?;
    let code = code.trim();
    if code.is_empty() {
        return None;
    }
    let probability = rest.trim_end().strip_suffix(')')?.parse().ok()?;
    Some(Detected { code: code.to_string(), probability })
}

/// Ask Whisper which language `audio_path` is spoken in.
pub async fn detect(audio_path: &Path, model_path: &Path) -> Result<Detected, String> {
    if !model_path.is_file() {
        return Err("Choose a Whisper model under Options first.".to_string());
    }
    let ffmpeg = binaries::require("ffmpeg")?;
    let whisper = binaries::resolve("whisper-cli")
        .or_else(|| binaries::resolve("whisper-cpp"))
        .ok_or_else(|| "whisper-cli is not installed.".to_string())?;

    // An input and no output: ffmpeg describes the file and exits 1, always.
    let probe = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(audio_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    let start = excerpt_start(parse_duration(&String::from_utf8_lossy(&probe.stderr)));

    let excerpt = excerpt_path();
    let heard = listen(&ffmpeg, &whisper, audio_path, model_path, &excerpt, start).await;
    let _ = std::fs::remove_file(&excerpt);
    heard
}

async fn listen(
    ffmpeg: &Path,
    whisper: &Path,
    audio_path: &Path,
    model_path: &Path,
    excerpt: &Path,
    start: f64,
) -> Result<Detected, String> {
    // Whisper only reads 16 kHz mono PCM.
    let cut = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{start:.3}"))
        .arg("-t")
        .arg(format!("{EXCERPT_SECONDS}"))
        .arg("-i")
        .arg(audio_path)
        .args(["-vn", "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(excerpt)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    if !cut.success() {
        return Err("ffmpeg could not read the file.".to_string());
    }

    // whisper.cpp logs its guess on stderr, and `-np` would silence it.
    let heard = Command::new(whisper)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(excerpt)
        .args(["-l", "auto", "-dl"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Could not run whisper: {e}"))?;
    String::from_utf8_lossy(&heard.stderr)
        .lines()
        .find_map(parse_detected)
        .ok_or_else(|| "Whisper could not tell which language this is.".to_string())
}

/// A name of its own for each excerpt: two detections may overlap, one for a
/// dialog and one left over from a dialog that has closed.
fn excerpt_path() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("yt-mp3-language-{}-{n}.wav", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_duration_ffmpeg_prints_for_an_input() {
        let header = "Input #0, mp3, from 'x.mp3':\n  Duration: 00:00:12.07, start: 0.023021, bitrate: 125 kb/s\n";
        assert_eq!(parse_duration(header), Some(12.07));
        assert_eq!(parse_duration("  Duration: 01:04:30.50, start: 0.000000"), Some(3870.5));
    }

    #[test]
    fn a_stream_of_unknown_length_has_no_duration() {
        assert_eq!(parse_duration("  Duration: N/A, start: 0.000000, bitrate: N/A"), None);
        assert_eq!(parse_duration("At least one output file must be specified"), None);
    }

    #[test]
    fn listens_a_third_of_the_way_in_so_an_intro_does_not_decide() {
        assert_eq!(excerpt_start(Some(3600.0)), 1200.0);
        assert_eq!(excerpt_start(Some(90.0)), 30.0);
    }

    #[test]
    fn never_starts_so_late_that_less_than_thirty_seconds_are_left() {
        // A third of 40 s is 13.3 s, which would leave 26.7 s.
        assert_eq!(excerpt_start(Some(40.0)), 10.0);
    }

    #[test]
    fn a_short_or_unmeasured_file_is_heard_from_the_start() {
        assert_eq!(excerpt_start(Some(30.0)), 0.0);
        assert_eq!(excerpt_start(Some(12.07)), 0.0);
        assert_eq!(excerpt_start(None), 0.0);
    }

    #[test]
    fn reads_whispers_guess_from_its_log_line() {
        let heard = parse_detected("whisper_full_with_state: auto-detected language: hr (p = 0.526195)")
            .expect("a guess");
        assert_eq!(heard.code, "hr");
        assert!((heard.probability - 0.526195).abs() < 1e-6);
    }

    #[test]
    fn ignores_whispers_other_chatter() {
        for line in [
            "whisper_init_from_file_with_params_no_state: loading model",
            "",
            "auto-detected language:  (p = 0.5)",
        ] {
            assert_eq!(parse_detected(line), None, "{line}");
        }
    }
}
