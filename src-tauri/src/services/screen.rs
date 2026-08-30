//! Reading what was on screen.
//!
//! Half of a screen-share lecture is not spoken. The speaker says "this one
//! here" and points at a class name, and the transcript keeps the pointing and
//! loses the name. This module recovers the names.
//!
//! Frames are taken only where the picture changes -- an hour of a static slide
//! is one screen, not 3600 -- and read with Apple's Vision OCR through a helper
//! binary embedded in the app.

use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::AppHandle;

use super::binaries;
use super::transcript::Screen;
use super::ytdlp::{emit, ProgressEvent};

/// The compiled Swift helper, built by build.rs.
const OCR_HELPER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ocr"));

/// Czech, because Serbian is not a recognition language and Czech shares the
/// diacritics: it reads "sledeća" where English produces "sledeéa".
const OCR_LANGUAGES: &[&str] = &["cs-CZ", "en-US"];

/// How different a frame must be to count as a new screen. Low, because a
/// settings panel changing one field is exactly what we are here for.
const SCENE_THRESHOLD: &str = "0.06";

/// A screen has to say something to earn a line in the transcript.
///
/// OCR leaves fragments behind -- "COnS", "iF st" -- which are new, and so
/// survive deduplication, but tell the reader nothing. Judging the screen as a
/// whole rather than line by line keeps short but real labels like "Autoplay"
/// when they arrive alongside other text.
const MIN_SCREEN_CHARS: usize = 16;

/// What was read from a video, kept beside it.
///
/// Reading an hour of screen costs about twelve minutes, and nothing about it
/// changes between runs. Caching turns "try a different layout" from a coffee
/// break into a second.
fn cache_path(video: &Path) -> PathBuf {
    video.with_extension("screens.json")
}

fn cached(video: &Path) -> Option<Vec<Screen>> {
    let cache = cache_path(video);
    let fresh = std::fs::metadata(&cache)
        .ok()?
        .modified()
        .ok()?
        >= std::fs::metadata(video).ok()?.modified().ok()?;

    fresh.then(|| serde_json::from_str(&std::fs::read_to_string(&cache).ok()?).ok())?
}

/// Read every distinct screen in a video, or recall what was read before.
pub fn read_screens(app: &AppHandle, video: &Path, work_dir: &Path) -> Result<Vec<Screen>, String> {
    if let Some(screens) = cached(video) {
        emit(app, ProgressEvent::stage("reading", Some("Screens already read".into())));
        return Ok(screens);
    }

    emit(app, ProgressEvent::stage("reading", Some("Looking for screen changes".into())));

    let frames = extract_frames(video, work_dir)?;
    let helper = install_helper(work_dir)?;
    let total = frames.len();

    let mut seen: Vec<String> = Vec::new();
    let mut screens = Vec::new();

    for (index, (at_ms, frame)) in frames.into_iter().enumerate() {
        emit(
            app,
            ProgressEvent {
                stage: "reading".to_string(),
                percent: Some((index as f32 / total.max(1) as f32) * 100.0),
                detail: Some(format!("Screen {} of {}", index + 1, total)),
            },
        );

        let lines = read_text(&helper, &frame);
        let _ = std::fs::remove_file(&frame);

        let fresh = fresh_lines(&lines, &seen);
        seen.extend(lines);

        if worth_keeping(&fresh) {
            screens.push(Screen { at_ms, lines: fresh });
        }
    }

    if let Ok(json) = serde_json::to_string(&screens) {
        // A failed cache write costs a re-read next time, nothing more.
        let _ = std::fs::write(cache_path(video), json);
    }

    Ok(screens)
}

/// Pull out one frame per visible change, with the time it appeared.
fn extract_frames(video: &Path, work_dir: &Path) -> Result<Vec<(u64, PathBuf)>, String> {
    let ffmpeg = binaries::require("ffmpeg")?;
    let frame_dir = work_dir.join("frames");
    let _ = std::fs::remove_dir_all(&frame_dir);
    std::fs::create_dir_all(&frame_dir)
        .map_err(|e| format!("Could not create the frames folder: {e}"))?;

    // `-skip_frame nokey` decodes only keyframes, which turns a 13x realtime
    // pass into a 48x one. Sampling every other second is far finer than any
    // lecture changes its screen.
    let output = Command::new(ffmpeg)
        .arg("-skip_frame")
        .arg("nokey")
        .arg("-i")
        .arg(video)
        .arg("-vf")
        .arg(format!(
            "fps=1/2,select='gt(scene,{SCENE_THRESHOLD})',scale=1400:-1,showinfo"
        ))
        .args(["-fps_mode", "vfr", "-q:v", "3"])
        .arg(frame_dir.join("frame_%04d.jpg"))
        .output()
        .map_err(|e| format!("Could not read the video: {e}"))?;

    let times = parse_frame_times(&String::from_utf8_lossy(&output.stderr));

    let mut files: Vec<PathBuf> = std::fs::read_dir(&frame_dir)
        .map_err(|e| format!("Could not list the frames: {e}"))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "jpg"))
        .collect();
    files.sort();

    Ok(times.into_iter().zip(files).collect())
}

/// ffmpeg's showinfo prints `pts_time:12.5` for every frame it lets through,
/// in the same order as the files it writes.
fn parse_frame_times(showinfo: &str) -> Vec<u64> {
    showinfo
        .split("pts_time:")
        .skip(1)
        .filter_map(|rest| {
            let value = rest.split_whitespace().next()?;
            value.parse::<f64>().ok().map(|seconds| (seconds * 1000.0) as u64)
        })
        .collect()
}

fn read_text(helper: &Path, frame: &Path) -> Vec<String> {
    let Ok(output) = Command::new(helper).arg(frame).args(OCR_LANGUAGES).output() else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(tidy_line)
        .filter(|line| line.chars().count() > 3)
        .collect()
}

/// Icons come through as stray marks around the words.
///
/// The narrow rule is deliberate: a leading token is only dropped when it is
/// one or two characters *and* contains a mark, which catches the "L*" and
/// "v:" that a misread icon leaves behind without touching real content like
/// "A/B testing" or "3 od 12".
fn tidy_line(line: &str) -> String {
    let trimmed = line
        .trim()
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim_end_matches(|c: char| !c.is_alphanumeric() && c != ')' && c != '.')
        .trim();

    match trimmed.split_once(char::is_whitespace) {
        Some((first, rest))
            if first.chars().count() <= 2 && first.chars().any(|c| !c.is_alphanumeric()) =>
        {
            rest.trim().to_string()
        }
        _ => trimmed.to_string(),
    }
}

/// Placeholder copy carries no information, and OCR reads it slightly
/// differently every time, so deduplication never catches it. Webflow demo
/// pages are full of it.
fn is_placeholder(line: &str) -> bool {
    const LOREM: &[&str] = &[
        "lorem ipsum", "dolor sit amet", "consectetur", "eiusmod", "incididunt",
        "labore et dolore", "aliquip", "commodo consequat", "ullamco", "laboris",
        "reprehenderit", "voluptate", "excepteur",
    ];
    let lower = line.to_lowercase();
    LOREM.iter().any(|fragment| lower.contains(fragment))
}

/// Keep only what has not been on screen before.
///
/// WHY: the application chrome is identical in every frame. A raw dump is
/// mostly the same navigator panel over and over, and the one line that
/// actually changed is lost in it.
fn fresh_lines(lines: &[String], seen: &[String]) -> Vec<String> {
    let mut fresh: Vec<String> = Vec::new();

    for line in lines {
        if is_placeholder(line) {
            continue;
        }

        // Substring rather than equality: OCR clips the same panel differently
        // as it scrolls, so "u-align-self-center" arrives as a piece of a line
        // already recorded.
        let already = seen.iter().chain(fresh.iter()).any(|other| contains_either_way(other, line));
        if !already {
            fresh.push(line.clone());
        }
    }

    fresh
}

fn contains_either_way(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_lowercase(), b.to_lowercase());
    a == b || (b.len() > 6 && a.contains(&b)) || (a.len() > 6 && b.contains(&a))
}

fn worth_keeping(lines: &[String]) -> bool {
    lines.iter().map(|line| line.chars().count()).sum::<usize>() >= MIN_SCREEN_CHARS
}

/// Write the embedded helper somewhere it can be run.
fn install_helper(work_dir: &Path) -> Result<PathBuf, String> {
    let path = work_dir.join("ocr");

    let current = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if current != OCR_HELPER.len() as u64 {
        std::fs::write(&path, OCR_HELPER)
            .map_err(|e| format!("Could not unpack the text reader: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Could not make the text reader runnable: {e}"))?;
        }
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_frame_times_out_of_showinfo() {
        let showinfo = "\
[Parsed_showinfo_3 @ 0x1] n:0 pts:1024 pts_time:34 duration:512
[Parsed_showinfo_3 @ 0x1] n:1 pts:2048 pts_time:56.5 duration:512";

        assert_eq!(parse_frame_times(showinfo), vec![34_000, 56_500]);
    }

    #[test]
    fn ignores_output_with_no_frames() {
        assert!(parse_frame_times("ffmpeg version 7.1\nno frames here").is_empty());
    }

    #[test]
    fn strips_the_junk_icons_leave_around_words() {
        assert_eq!(tidy_line("~ • page_wrap"), "page_wrap");
        assert_eq!(tidy_line("  L* Design ~ "), "Design");
        assert_eq!(tidy_line("D 140 x 140px"), "D 140 x 140px", "a bare letter is not an icon");
        assert_eq!(tidy_line("A/B testing"), "A/B testing", "real content survives");
        assert_eq!(tidy_line("Global Styles"), "Global Styles");
    }

    #[test]
    fn keeps_only_what_has_not_been_shown_before() {
        let seen = vec!["Navigator".to_string(), "page_wrap".to_string()];
        let lines = vec![
            "Navigator".to_string(),
            "Visual Video".to_string(),
            "navigator".to_string(), // the same panel, differently cased
            "Autoplay".to_string(),
        ];

        assert_eq!(fresh_lines(&lines, &seen), vec!["Visual Video", "Autoplay"]);
    }

    #[test]
    fn placeholder_copy_never_counts_as_content() {
        let lines = vec![
            "Lorem ipsum dolor sit amet, consectetur".to_string(),
            "u-align-self-center".to_string(),
            "labore et dolore magna aliqua".to_string(),
        ];
        assert_eq!(fresh_lines(&lines, &[]), vec!["u-align-self-center"]);
    }

    #[test]
    fn a_fragment_of_something_already_seen_is_not_new() {
        let seen = vec!["Visual Video component settings".to_string()];
        let lines = vec!["Visual Video component".to_string(), "Autoplay".to_string()];
        assert_eq!(fresh_lines(&lines, &seen), vec!["Autoplay"]);
    }

    #[test]
    fn a_short_line_is_not_swallowed_by_a_longer_one() {
        let seen = vec!["Grid settings".to_string()];
        assert_eq!(fresh_lines(&[String::from("Grid")], &seen), vec!["Grid"]);
    }

    #[test]
    fn a_screen_of_ocr_crumbs_is_dropped() {
        assert!(!worth_keeping(&["COnS".to_string()]));
        assert!(!worth_keeping(&["iF st".to_string(), "Play".to_string()]));
        assert!(!worth_keeping(&[]));
    }

    #[test]
    fn a_screen_that_actually_says_something_is_kept() {
        assert!(worth_keeping(&["Visual Video".to_string(), "Autoplay".to_string()]));
        assert!(worth_keeping(&["clamp(1.125 * 1rem, ((1.125".to_string()]));
    }

    #[test]
    fn a_screen_that_repeats_itself_contributes_nothing() {
        let seen = vec!["Navigator".to_string()];
        assert!(fresh_lines(&[String::from("Navigator")], &seen).is_empty());
    }
}
