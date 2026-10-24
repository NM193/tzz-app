//! An MP3 of a file already on disk, for that file's folder in the library.
//!
//! The original is never touched. An MP3 is copied as it is -- encoding it
//! again would only lose quality -- and anything else, a video, a WAV, an
//! M4A, is encoded at the quality chosen for downloads.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use super::binaries;
use super::cancel;

/// Whether `source` has to be encoded, or can simply be copied.
pub fn needs_encoding(source: &Path) -> bool {
    !source
        .extension()
        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("mp3"))
}

/// LAME's settings for the UI's choice, matching what a download gets.
pub fn quality_args(choice: Option<&str>) -> [&'static str; 2] {
    match choice {
        Some("320") => ["-b:a", "320k"], // constant bitrate
        _ => ["-q:a", "0"],              // VBR V0, ~245 kbps average
    }
}

/// Put an MP3 of `source` at `destination`. Claimed first, so a Stop takes
/// back a half-written file.
pub async fn keep(source: &Path, destination: &Path, quality: Option<&str>) -> Result<PathBuf, String> {
    cancel::claim(destination);
    if !needs_encoding(source) {
        tokio::fs::copy(source, destination)
            .await
            .map_err(|e| format!("Could not copy the audio: {e}"))?;
        return Ok(destination.to_path_buf());
    }

    let ffmpeg = binaries::require("ffmpeg")?;
    let mut child = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source)
        .args(["-vn", "-map", "0:a:0", "-c:a", "libmp3lame"])
        .args(quality_args(quality))
        .arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    let tracked = cancel::track(child.id());
    let status = child.wait().await.map_err(|e| format!("Could not run ffmpeg: {e}"))?;
    drop(tracked);
    cancel::check()?;
    if !status.success() {
        return Err("ffmpeg could not make an MP3 of this file.".to_string());
    }
    Ok(destination.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_mp3_is_copied_as_it_is() {
        assert!(!needs_encoding(Path::new("/x/Recording 2026-10-24 16-20.mp3")));
        assert!(!needs_encoding(Path::new("/x/LOUD.MP3")));
    }

    #[test]
    fn anything_else_is_encoded() {
        for name in ["lecture.mp4", "talk.m4a", "raw.wav", "noext"] {
            assert!(needs_encoding(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn the_quality_matches_a_downloads() {
        assert_eq!(quality_args(Some("320")), ["-b:a", "320k"]);
        assert_eq!(quality_args(Some("v0")), ["-q:a", "0"]);
        assert_eq!(quality_args(None), ["-q:a", "0"]);
    }
}
