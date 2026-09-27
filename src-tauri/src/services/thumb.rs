//! One picture per folder, so the library reads as shelves rather than a list
//! of names.
//!
//! The picture is made once and kept as `poster.jpg` beside everything else.
//! Whatever the folder holds, something can stand for it:
//!
//! 1. a cover yt-dlp already wrote (the real YouTube thumbnail -- best);
//! 2. a frame from the video, taken a little way in, past the title card;
//! 3. the waveform of the audio, which at least shows where the talking is;
//! 4. the cover fetched from YouTube, for a folder that kept only a document
//!    -- its name still carries the video id;
//! 5. nothing, and the screen falls back to an icon.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::binaries;

pub const POSTER: &str = "poster.jpg";

const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac"];
/// Covers yt-dlp writes. It converts to jpg when it can, but not always.
const COVER: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Wide enough for a four-column grid on a Retina screen, small enough that a
/// hundred of them cost nothing.
const WIDTH: &str = "640";

pub fn existing(folder: &Path) -> Option<PathBuf> {
    let poster = folder.join(POSTER);
    poster.is_file().then_some(poster)
}

/// The folder's picture, made if it is not there yet.
///
/// Never an error: a library with no pictures is still a library.
pub fn ensure(folder: &Path) -> Option<PathBuf> {
    if let Some(poster) = existing(folder) {
        return Some(poster);
    }
    let poster = folder.join(POSTER);

    if let Some(cover) = pick(folder, COVER) {
        if convert(&cover, &poster) {
            // The source was yt-dlp's own file; it has done its job.
            let _ = std::fs::remove_file(&cover);
            return Some(poster);
        }
    }
    if let Some(video) = pick(folder, VIDEO) {
        if frame(&video, &poster) {
            return Some(poster);
        }
    }
    if let Some(audio) = pick(folder, AUDIO) {
        if waveform(&audio, &poster) {
            return Some(poster);
        }
    }
    if from_youtube(folder, &poster) {
        return Some(poster);
    }
    None
}

/// The video id a job folder is named after: `Some title [dQw4w9WgXcQ]`.
fn video_id(folder: &Path) -> Option<String> {
    let name = folder.file_name()?.to_str()?.trim_end();
    let inside = name.strip_suffix(']')?.rsplit_once('[')?.1;
    let ok = !inside.is_empty()
        && inside.len() <= 20
        && inside.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| inside.to_string())
}

/// Ask YouTube for the cover of the video this folder came from.
///
/// The last resort, and the only one that needs the network: a folder that
/// kept nothing but a transcript has no picture of its own, yet the lecture it
/// came from still has one. Failing costs a retry on the next launch.
fn from_youtube(folder: &Path, poster: &Path) -> bool {
    let Some(id) = video_id(folder) else {
        return false;
    };
    let Ok(ytdlp) = binaries::require("yt-dlp") else {
        return false;
    };

    let work = std::env::temp_dir().join(format!("tzz-cover-{id}"));
    let _ = std::fs::remove_dir_all(&work);
    if std::fs::create_dir_all(&work).is_err() {
        return false;
    }

    let fetched = Command::new(ytdlp)
        .args(["--no-playlist", "--skip-download", "--write-thumbnail"])
        .args(["--convert-thumbnails", "jpg"])
        .arg("--output")
        .arg(work.join("%(id)s.%(ext)s"))
        .arg(format!("https://www.youtube.com/watch?v={id}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    let made = fetched
        .then(|| pick(&work, COVER))
        .flatten()
        .is_some_and(|cover| convert(&cover, poster));

    let _ = std::fs::remove_dir_all(&work);
    made
}

/// The first file with one of these extensions. `poster.jpg` is never a source.
fn pick(folder: &Path, extensions: &[&str]) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|name| name != POSTER))
        .filter(|path| {
            path.extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .is_some_and(|e| extensions.contains(&e.as_str()))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

fn run(args: &[&str]) -> bool {
    let Ok(ffmpeg) = binaries::require("ffmpeg") else {
        return false;
    };
    Command::new(ffmpeg)
        .arg("-y")
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn wrote(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}

fn convert(source: &Path, poster: &Path) -> bool {
    run(&[
        "-i",
        &source.to_string_lossy(),
        "-vf",
        &format!("scale={WIDTH}:-2"),
        "-frames:v",
        "1",
        "-q:v",
        "3",
        &poster.to_string_lossy(),
    ]) && wrote(poster)
}

/// A frame from a little way in: the first second of a lecture is usually a
/// black title card or a countdown.
fn frame(video: &Path, poster: &Path) -> bool {
    for seek in ["30", "8", "0"] {
        let ok = run(&[
            "-ss",
            seek,
            "-i",
            &video.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={WIDTH}:-2"),
            "-q:v",
            "3",
            &poster.to_string_lossy(),
        ]);
        if ok && wrote(poster) {
            return true;
        }
    }
    false
}

/// Audio has no picture, so draw the sound itself.
fn waveform(audio: &Path, poster: &Path) -> bool {
    run(&[
        "-i",
        &audio.to_string_lossy(),
        "-filter_complex",
        // Split channels would draw two bands; one reads better this small.
        "aformat=channel_layouts=mono,showwavespic=s=640x400:colors=#8a827b",
        "-frames:v",
        "1",
        &poster.to_string_lossy(),
    ]) && wrote(poster)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tzz-thumb-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn an_existing_poster_is_reused() {
        let folder = temp("existing");
        std::fs::write(folder.join(POSTER), b"x").unwrap();
        assert_eq!(existing(&folder), Some(folder.join(POSTER)));
    }

    #[test]
    fn the_poster_is_never_its_own_source() {
        let folder = temp("self");
        std::fs::write(folder.join(POSTER), b"x").unwrap();
        assert_eq!(pick(&folder, COVER), None);
    }

    #[test]
    fn a_cover_is_preferred_and_found_by_extension() {
        let folder = temp("cover");
        std::fs::write(folder.join("Lecture [abc].webp"), b"x").unwrap();
        std::fs::write(folder.join("Lecture [abc].mp3"), b"x").unwrap();
        assert!(pick(&folder, COVER).is_some());
        assert!(pick(&folder, VIDEO).is_none());
        assert!(pick(&folder, AUDIO).is_some());
    }

    #[test]
    fn reads_the_video_id_out_of_a_folder_name() {
        assert_eq!(
            video_id(Path::new("/x/Lumos V2.1 Crash Course [GPRcAZLuT3U]")),
            Some("GPRcAZLuT3U".to_string())
        );
        // A title with its own brackets must not confuse the last one.
        assert_eq!(
            video_id(Path::new("/x/Lecture [part 2] [abc-123_X]")),
            Some("abc-123_X".to_string())
        );
    }

    #[test]
    fn a_folder_without_an_id_asks_youtube_for_nothing() {
        assert_eq!(video_id(Path::new("/x/Recording 2026-09-27")), None);
        assert_eq!(video_id(Path::new("/x/Lecture [not an id]")), None);
    }

    #[test]
    fn a_folder_with_nothing_to_show_makes_no_poster() {
        // No id in the name either, so nothing to ask YouTube for.
        let folder = temp("empty");
        std::fs::write(folder.join("notes.md"), b"x").unwrap();
        assert_eq!(ensure(&folder), None);
    }
}
