//! Orchestration of yt-dlp: metadata probe + audio download with live progress.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::binaries;
use super::cancel;

/// Sentinel we prepend to yt-dlp's progress template so we can pick our own
/// lines out of the mixed stdout/stderr stream without fragile regex on
/// yt-dlp's human-readable output (which changes between releases).
const PROGRESS_TAG: &str = "__PROGRESS__";

pub const PROGRESS_EVENT: &str = "download://progress";

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    /// "probing" | "downloading" | "converting" | "transcribing" | "done"
    pub stage: String,
    pub percent: Option<f32>,
    pub detail: Option<String>,
}

impl ProgressEvent {
    pub fn stage(stage: &str, detail: Option<String>) -> Self {
        Self { stage: stage.to_string(), percent: None, detail }
    }
}

pub fn emit(app: &AppHandle, event: ProgressEvent) {
    // Progress is advisory: a failed emit must never abort the job.
    let _ = app.emit(PROGRESS_EVENT, event);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMeta {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration_seconds: Option<f64>,
    pub thumbnail: Option<String>,
    /// Languages with human-made subtitles.
    pub manual_subtitle_langs: Vec<String>,
    /// Languages with YouTube auto-generated captions.
    pub auto_caption_langs: Vec<String>,
    /// The uploader's own chapters, when there are any.
    pub chapters: Vec<Chapter>,
}

/// A section of the video, as the uploader marked it out.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub title: String,
    #[serde(rename = "start_time", alias = "startSeconds")]
    pub start_seconds: f64,
}

#[derive(Deserialize)]
struct RawMeta {
    id: String,
    title: String,
    uploader: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    #[serde(default)]
    subtitles: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    automatic_captions: serde_json::Map<String, serde_json::Value>,
    chapters: Option<Vec<Chapter>>,
}

/// One video inside a playlist, as the flat listing reports it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub title: String,
    pub items: Vec<PlaylistItem>,
}

#[derive(Deserialize)]
struct RawPlaylist {
    title: Option<String>,
    #[serde(default)]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    id: Option<String>,
    title: Option<String>,
}

/// A playlist beyond this is not a course, it is a channel dump. Taking the
/// first hundred keeps the link field usable and says so.
pub const PLAYLIST_LIMIT: usize = 100;

/// Read what is in a playlist without touching a single video.
///
/// `--flat-playlist` is the whole point: it asks YouTube once for the listing
/// instead of extracting every video, so a fifty-lecture course comes back in
/// a second or two rather than a minute.
pub async fn probe_playlist(url: &str) -> Result<Playlist, String> {
    let ytdlp = binaries::require("yt-dlp")?;

    let output = Command::new(ytdlp)
        .args(["--flat-playlist", "--skip-download", "--dump-single-json", url])
        .output()
        .await
        .map_err(|e| format!("Could not run yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(tail_of(&String::from_utf8_lossy(&output.stderr)));
    }

    let raw: RawPlaylist = serde_json::from_slice(&output.stdout)
        .map_err(|_| "That link did not come back as a playlist.".to_string())?;

    Ok(Playlist {
        title: raw.title.unwrap_or_else(|| "Playlist".to_string()),
        items: playlist_items(raw.entries),
    })
}

/// Keep the videos that can actually be fetched.
///
/// A playlist carries its deleted and private entries too, with no id or a
/// placeholder title. Queuing those would be ten failures the user has to read.
fn playlist_items(entries: Vec<RawEntry>) -> Vec<PlaylistItem> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let id = entry.id?;
            let title = entry.title?;
            if id.is_empty() || is_unavailable(&title) {
                return None;
            }
            Some(PlaylistItem {
                url: format!("https://www.youtube.com/watch?v={id}"),
                id,
                title,
            })
        })
        .take(PLAYLIST_LIMIT)
        .collect()
}

fn is_unavailable(title: &str) -> bool {
    matches!(
        title.trim(),
        "[Private video]" | "[Deleted video]" | "[Unavailable video]" | ""
    )
}

/// Read video metadata without downloading anything.
/// Also tells the UI up front whether a transcript can come from captions.
pub async fn probe(url: &str) -> Result<VideoMeta, String> {
    let ytdlp = binaries::require("yt-dlp")?;

    let child = Command::new(ytdlp)
        .args(["--no-playlist", "--skip-download", "--dump-single-json", url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not run yt-dlp: {e}"))?;
    let tracked = cancel::track(child.id());
    // No stop check here: the title lookup for a pasted link runs outside any
    // job, when the flag may still be raised from the last stop.
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| format!("Could not run yt-dlp: {e}"))?;
    drop(tracked);

    if !output.status.success() {
        return Err(tail_of(&String::from_utf8_lossy(&output.stderr)));
    }

    let raw: RawMeta = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Could not read video metadata: {e}"))?;

    Ok(VideoMeta {
        id: raw.id,
        title: raw.title,
        uploader: raw.uploader,
        duration_seconds: raw.duration,
        thumbnail: raw.thumbnail,
        manual_subtitle_langs: raw.subtitles.keys().cloned().collect(),
        auto_caption_langs: raw.automatic_captions.keys().cloned().collect(),
        chapters: raw.chapters.unwrap_or_default(),
    })
}

pub struct DownloadOptions<'a> {
    pub url: &'a str,
    pub out_dir: &'a Path,
    pub video_id: &'a str,
    pub want_subtitles: bool,
    /// Ordered language preference, e.g. ["sr", "en"].
    pub subtitle_langs: &'a [String],
    /// yt-dlp's --audio-quality value: "0" for LAME VBR V0, "320K" for CBR 320.
    pub audio_quality: &'a str,
}

pub struct DownloadOutput {
    pub audio_path: PathBuf,
    pub subtitle_paths: Vec<PathBuf>,
    /// Problems that did not cost us the audio.
    pub warnings: Vec<String>,
}

/// Download best audio, convert to MP3, optionally pull subtitles as SRT.
pub async fn download_audio(
    app: &AppHandle,
    opts: DownloadOptions<'_>,
) -> Result<DownloadOutput, String> {
    let ytdlp = binaries::require("yt-dlp")?;
    binaries::require("ffmpeg")?; // yt-dlp needs it for the MP3 conversion

    std::fs::create_dir_all(opts.out_dir)
        .map_err(|e| format!("Could not create output folder: {e}"))?;

    let mut args: Vec<String> = vec![
        "--no-playlist".into(),
        "--newline".into(),
        "--no-part".into(),
        // Structured progress we can parse reliably.
        "--progress-template".into(),
        format!("download:{PROGRESS_TAG} download %(progress._percent_str)s %(progress._eta_str)s"),
        "--progress-template".into(),
        format!("postprocess:{PROGRESS_TAG} postprocess %(progress._percent_str)s"),
        // YouTube answers a burst from one address with a 403 that is gone a
        // moment later. Without these a job of ten lectures loses one for no
        // lasting reason.
        "--retries".into(),
        "10".into(),
        "--extractor-retries".into(),
        "3".into(),
        "--fragment-retries".into(),
        "10".into(),
        "--extract-audio".into(),
        "--audio-format".into(),
        "mp3".into(),
        "--audio-quality".into(),
        opts.audio_quality.to_string(),
        "--embed-metadata".into(),
        // The library shows a picture per folder; this is the best one there
        // is, and it costs one extra request during a download already running.
        "--write-thumbnail".into(),
        "--convert-thumbnails".into(),
        "jpg".into(),
        // Filename carries the video id so we can find the results deterministically.
        "--output".into(),
        "%(title).120B [%(id)s].%(ext)s".into(),
        "--paths".into(),
        opts.out_dir.to_string_lossy().into_owned(),
    ];

    if opts.want_subtitles {
        args.extend([
            "--write-subs".into(),
            "--write-auto-subs".into(),
            "--sub-langs".into(),
            subtitle_langs_arg(opts.subtitle_langs),
            "--convert-subs".into(),
            "srt".into(),
            // One request per second. YouTube answers a burst with HTTP 429.
            "--sleep-requests".into(),
            "1".into(),
            // Captions are a bonus, the audio is the point. Without this a
            // refused subtitle aborts the run before the format is fetched,
            // and we end up with nothing at all.
            "--ignore-errors".into(),
        ]);
    }

    args.push(opts.url.to_string());

    let mut child = Command::new(ytdlp)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not start yt-dlp: {e}"))?;
    let _tracked = cancel::track(child.id());

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let mut out_lines = BufReader::new(stdout).lines();
    let mut err_lines = BufReader::new(stderr).lines();

    // Keep the last lines so a failure can report something actionable.
    let mut log: Vec<String> = Vec::new();
    let (mut out_done, mut err_done) = (false, false);

    while !(out_done && err_done) {
        tokio::select! {
            line = out_lines.next_line(), if !out_done => match line {
                Ok(Some(line)) => handle_line(app, &line, &mut log),
                _ => out_done = true,
            },
            line = err_lines.next_line(), if !err_done => match line {
                Ok(Some(line)) => handle_line(app, &line, &mut log),
                _ => err_done = true,
            },
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("yt-dlp did not finish cleanly: {e}"))?;
    cancel::check()?;

    if !status.success() {
        // A failed subtitle fetch must not throw away an MP3 that is already
        // on disk. Only a run with no audio to show for it is a real failure.
        return match collect_outputs(opts.out_dir, opts.video_id) {
            Ok(mut output) => {
                output.warnings.push(failure_summary(&log));
                Ok(output)
            }
            Err(_) => Err(tail_of(&log.join("\n"))),
        };
    }

    collect_outputs(opts.out_dir, opts.video_id)
}

/// Download the video stream on its own, for reading the screen.
///
/// Kept apart from `download_audio` rather than folded into it: the audio path
/// is the one that always runs, and it is not worth risking for a feature that
/// is off by default. `bv*` is video only, so nothing is fetched twice.
pub async fn download_video(
    app: &AppHandle,
    url: &str,
    out_dir: &Path,
    video_id: &str,
) -> Result<PathBuf, String> {
    let ytdlp = binaries::require("yt-dlp")?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("Could not create a folder: {e}"))?;

    // A video from an earlier run is reused rather than fetched again. This is
    // not only faster: with `--no-part` yt-dlp writes straight to the final
    // name, so it mistakes a finished file for an interrupted download and
    // fails trying to resume past its end (HTTP 416).
    if let Some(existing) = find_video(out_dir, video_id) {
        return Ok(existing);
    }

    emit(app, ProgressEvent::stage("downloading", Some("Video, to read the screen".into())));

    let mut child = Command::new(ytdlp)
        .args(["--no-playlist", "--newline", "--no-part", "-f", "bv*"])
        .args(["--retries", "10", "--extractor-retries", "3", "--fragment-retries", "10"])
        .arg("--progress-template")
        .arg(format!(
            "download:{PROGRESS_TAG} download %(progress._percent_str)s %(progress._eta_str)s"
        ))
        // Same shape as the MP3, so a folder reads as one lecture in two forms
        // rather than a title and a mystery id.
        .arg("--output")
        .arg(out_dir.join("%(title).120B [%(id)s].%(ext)s"))
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not download the video: {e}"))?;
    let _tracked = cancel::track(child.id());

    // Same progress line as the audio download, so the meter keeps moving
    // through what is usually the longest wait in the whole job.
    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        let mut log = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            handle_line(app, &line, &mut log);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("The video download did not finish cleanly: {e}"))?;
    cancel::check()?;

    if !status.success() {
        return Err("The video could not be downloaded for screen reading.".to_string());
    }

    find_video(out_dir, video_id).ok_or_else(|| "yt-dlp finished but wrote no video.".to_string())
}

/// The video for this id, by either name it has ever had.
///
/// Older downloads were named by id alone; newer ones carry the title. Both
/// are recognised so nothing already on disk gets fetched again.
fn find_video(dir: &Path, video_id: &str) -> Option<PathBuf> {
    const VIDEO: &[&str] = &["mp4", "webm", "mkv", "mov"];
    let marker = format!("[{video_id}]");

    std::fs::read_dir(dir).ok()?.flatten().map(|entry| entry.path()).find(|path| {
        let stem = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        VIDEO.contains(&ext.as_str()) && (stem == video_id || stem.ends_with(&marker))
    })
}

fn handle_line(app: &AppHandle, line: &str, log: &mut Vec<String>) {
    if let Some(rest) = line.trim().strip_prefix(PROGRESS_TAG) {
        let mut parts = rest.split_whitespace();
        let kind = parts.next().unwrap_or("download");
        let percent = parts.next().and_then(parse_percent);
        let eta = parts.next().map(|s| format!("ETA {s}"));

        emit(
            app,
            ProgressEvent {
                stage: if kind == "postprocess" { "converting".into() } else { "downloading".into() },
                percent,
                detail: eta,
            },
        );
        return;
    }

    log.push(line.to_string());
    if log.len() > 40 {
        log.remove(0);
    }
}

fn parse_percent(raw: &str) -> Option<f32> {
    raw.trim().trim_end_matches('%').parse::<f32>().ok()
}

/// Build the --sub-langs value.
///
/// WHY not `sr.*`: the wildcard also matches YouTube's machine-translated
/// tracks (`sr-en-j3PyPqV-e1s` and friends), which turned one download into
/// six and earned us an HTTP 429. Asking for the language and its original
/// track is enough -- `pick_subtitle` chooses between them afterwards.
fn subtitle_langs_arg(langs: &[String]) -> String {
    langs
        .iter()
        .flat_map(|l| [l.clone(), format!("{l}-orig")])
        .collect::<Vec<_>>()
        .join(",")
}

/// Turn yt-dlp's own error into a plain sentence for the warnings list.
fn failure_summary(log: &[String]) -> String {
    let error = log.iter().rev().find(|line| line.contains("ERROR:"));

    match error {
        Some(line) if line.contains("429") => {
            "YouTube rate-limited the subtitles for this video. The MP3 was saved; \
             the transcript came from Whisper instead, if a model is set."
                .to_string()
        }
        Some(line) => {
            let detail = line.trim().split("ERROR: ").last().unwrap_or(line).trim();
            format!("The audio was saved, but part of the job failed: {detail}")
        }
        None => "The audio was saved, but yt-dlp reported a problem.".to_string(),
    }
}

/// Find what the run produced. We match on the video id embedded in the
/// filename rather than predicting yt-dlp's own title sanitisation.
fn collect_outputs(dir: &Path, video_id: &str) -> Result<DownloadOutput, String> {
    let marker = format!("[{video_id}]");
    let mut audio_path: Option<PathBuf> = None;
    let mut subtitle_paths: Vec<PathBuf> = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Could not read output folder: {e}"))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.contains(&marker) {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("mp3") => audio_path = Some(path),
            Some("srt") => subtitle_paths.push(path),
            _ => {}
        }
    }

    let audio_path = audio_path.ok_or_else(|| {
        "yt-dlp finished but no MP3 was written. Check that ffmpeg is installed.".to_string()
    })?;

    Ok(DownloadOutput { audio_path, subtitle_paths, warnings: Vec::new() })
}

/// yt-dlp errors are verbose; the useful part is at the end.
/// What to show when a job produced nothing.
///
/// yt-dlp narrates everything it does, so the tail of its output is mostly
/// housekeeping -- files it deleted, thumbnails it converted -- with the one
/// line that matters buried in it. Only the errors are worth reading, and a
/// 403 gets named, because it means "try again" rather than "this is broken".
fn tail_of(text: &str) -> String {
    let errors: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("ERROR:"))
        .map(|line| line.trim_start_matches("ERROR:").trim())
        .collect();

    if errors.iter().any(|line| line.contains("403")) {
        return "YouTube refused the download (403). It does that to a burst of \
                requests and stops within a minute -- try again."
            .to_string();
    }
    if errors.iter().any(|line| line.contains("429")) {
        return "YouTube rate-limited this address (429). Wait a few minutes and \
                try again."
            .to_string();
    }

    match errors.last() {
        Some(line) => line.to_string(),
        None => "yt-dlp failed without saying why.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_is_named_rather_than_quoted() {
        let log = "[download] Destination: x.webm\n\
                   Deleting original file x.vtt (pass -k to keep)\n\
                   ERROR: unable to download video data: HTTP Error 403: Forbidden";
        assert!(tail_of(log).contains("try again"), "{}", tail_of(log));
        assert!(!tail_of(log).contains("Deleting"), "{}", tail_of(log));
    }

    #[test]
    fn housekeeping_is_never_shown_as_the_reason() {
        let log = "WARNING: Skipping embedding sr subtitle\n\
                   [ThumbnailsConvertor] Converting thumbnail\n\
                   ERROR: Requested format is not available";
        assert_eq!(tail_of(log), "Requested format is not available");
    }

    #[test]
    fn something_has_to_be_said_even_with_no_error_line() {
        assert_eq!(tail_of("[download] 100%"), "yt-dlp failed without saying why.");
    }

    #[test]
    fn asks_for_each_language_and_its_original_track_only() {
        let langs = vec!["sr".to_string(), "en".to_string()];
        assert_eq!(subtitle_langs_arg(&langs), "sr,sr-orig,en,en-orig");
    }

    #[test]
    fn explains_rate_limiting_in_plain_words() {
        let log = vec![
            "[youtube] Downloading m3u8 information".to_string(),
            "ERROR: Unable to download video subtitles for 'sr': HTTP Error 429: Too Many Requests"
                .to_string(),
        ];
        let summary = failure_summary(&log);
        assert!(summary.contains("rate-limited"), "{summary}");
        assert!(!summary.contains("429"), "the raw code should not reach the user: {summary}");
    }

    #[test]
    fn passes_other_errors_through_without_the_prefix() {
        let log = vec!["ERROR: Video unavailable".to_string()];
        assert!(failure_summary(&log).ends_with("Video unavailable"));
    }
}

#[cfg(test)]
mod video_tests {
    use super::*;

    #[test]
    fn finds_a_video_left_by_an_earlier_run() {
        let dir = std::env::temp_dir().join("yt-mp3-test-video");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("abc123.mp4"), b"x").unwrap();
        std::fs::write(dir.join("Something else.mp3"), b"x").unwrap();

        assert_eq!(find_video(&dir, "abc123"), Some(dir.join("abc123.mp4")));
        assert_eq!(find_video(&dir, "nothing"), None);

        // The newer naming carries the title; the id sits in brackets.
        std::fs::write(dir.join("A Lecture [xyz789].webm"), b"x").unwrap();
        assert_eq!(find_video(&dir, "xyz789"), Some(dir.join("A Lecture [xyz789].webm")));

        // An MP3 with the same id is not the video.
        std::fs::write(dir.join("A Lecture [mp3only].mp3"), b"x").unwrap();
        assert_eq!(find_video(&dir, "mp3only"), None);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod playlist_tests {
    use super::*;

    fn entry(id: &str, title: &str) -> RawEntry {
        RawEntry { id: Some(id.to_string()), title: Some(title.to_string()) }
    }

    #[test]
    fn builds_a_watch_link_for_every_video() {
        let items = playlist_items(vec![entry("abc", "Lecture 1"), entry("def", "Lecture 2")]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].url, "https://www.youtube.com/watch?v=abc");
        assert_eq!(items[1].title, "Lecture 2");
    }

    #[test]
    fn leaves_out_what_cannot_be_fetched() {
        let items = playlist_items(vec![
            entry("abc", "Lecture 1"),
            entry("xxx", "[Private video]"),
            entry("yyy", "[Deleted video]"),
            RawEntry { id: None, title: Some("no id".into()) },
            entry("def", "Lecture 2"),
        ]);
        let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, vec!["Lecture 1", "Lecture 2"]);
    }

    #[test]
    fn a_channel_dump_is_cut_to_the_limit() {
        let many: Vec<RawEntry> =
            (0..250).map(|n| entry(&format!("id{n}"), &format!("Video {n}"))).collect();
        assert_eq!(playlist_items(many).len(), PLAYLIST_LIMIT);
    }
}
