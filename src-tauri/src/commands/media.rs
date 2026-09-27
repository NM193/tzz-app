//! Tauri commands = the only surface the frontend can reach.
//! Keep them thin: validate input, call a service, shape the response.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::services::binaries::{self, DependencyStatus};
use crate::services::cancel;
use crate::services::combine::{self, Section};
use crate::services::pdf;
use crate::services::screen;
use crate::services::thumb;
use crate::services::transcript::{self, Screen, Transcript, TranscriptHeader};
use crate::services::ytdlp::{self, DownloadOptions, ProgressEvent, VideoMeta};

/// Containers ffmpeg handles in practice; whisper never sees them directly.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "wav", "flac", "ogg", "opus", "aac", "mp4", "mov", "mkv", "webm",
];

fn supported_extension(path: &std::path::Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|ext| AUDIO_EXTENSIONS.contains(&ext.as_str()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRequest {
    pub url: String,
    /// Absolute folder path. Defaults to the library when empty.
    pub out_dir: Option<String>,
    /// Keep the MP3. Defaults to true; the audio may still be fetched for a
    /// transcript and thrown away afterwards.
    pub want_audio: Option<bool>,
    pub want_transcript: bool,
    /// Ordered language preference for captions, e.g. ["sr", "en"].
    pub transcript_langs: Vec<String>,
    /// "v0" (LAME VBR ~245 kbps) or "320" (CBR 320 kbps).
    pub audio_quality: Option<String>,
    /// Absolute path to a ggml whisper model. Enables the fallback when set.
    pub whisper_model_path: Option<String>,
    /// "md" | "pdf" | "both". Defaults to markdown.
    pub transcript_format: Option<String>,
    /// Also read the text that appears on screen.
    pub read_screen: Option<bool>,
    /// Keep the downloaded video instead of deleting it after reading.
    pub keep_video: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileRequest {
    /// Absolute path to an audio or video file already on disk.
    pub path: String,
    pub whisper_model_path: Option<String>,
    /// Whisper language hint. `None` means let it detect.
    pub language: Option<String>,
    /// "md" | "pdf" | "both". Defaults to markdown.
    pub transcript_format: Option<String>,
    /// Also read the text that appears on screen, for a video file.
    pub read_screen: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResult {
    pub title: String,
    /// Where everything for this job was written. Always present, so there is
    /// always somewhere to open even when nothing else is.
    pub folder: String,
    /// Absent when the user asked for the video alone.
    pub audio_path: Option<String>,
    /// Present when the video was kept.
    pub video_path: Option<String>,
    pub transcript: Option<Transcript>,
    /// Non-fatal problems, e.g. audio saved but transcript unavailable.
    pub warnings: Vec<String>,
}

/// Where ggml models are expected to sit, relative to the home folder.
const MODEL_DIRS: &[&str] = &["Library/Application Support/yt-mp3/models", ".yt-mp3/models"];

/// Suggest a Whisper model so the user does not have to go hunting for one.
///
/// Same spirit as `binaries::resolve`: probe the well-known spot instead of
/// asking. Returns `None` when nothing is there, which leaves the app on
/// captions only -- exactly what it did before.
#[tauri::command]
pub fn default_whisper_model() -> Option<String> {
    let home = dirs_home().ok()?;
    let dirs: Vec<PathBuf> = MODEL_DIRS.iter().map(|d| home.join(d)).collect();
    largest_model_in(&dirs).map(|p| p.to_string_lossy().into_owned())
}

/// Biggest `.bin` wins: a larger ggml file means a more capable model, which is
/// the one to prefer when several are sitting side by side.
fn largest_model_in(dirs: &[PathBuf]) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("bin")) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if best.as_ref().is_none_or(|(best_size, _)| size > *best_size) {
                best = Some((size, path));
            }
        }
    }

    best.map(|(_, path)| path)
}

#[tauri::command]
pub fn check_dependencies() -> Vec<DependencyStatus> {
    binaries::status_for(&["yt-dlp", "ffmpeg", "whisper-cli"])
}

#[tauri::command]
pub async fn probe_video(url: String) -> Result<VideoMeta, String> {
    let url = normalize_url(&url)?;
    ytdlp::probe(&url).await
}

#[tauri::command]
pub async fn run_job(app: AppHandle, request: JobRequest) -> Result<JobResult, String> {
    let started = cancel::begin();
    let result = run_job_inner(&app, request).await;
    cancel::finish(result, started)
}

/// Stop the running job. Its half-written files are deleted before the
/// job's own call returns.
#[tauri::command]
pub fn cancel_job() {
    cancel::cancel();
}

async fn run_job_inner(app: &AppHandle, request: JobRequest) -> Result<JobResult, String> {
    let app = app.clone();
    let url = normalize_url(&request.url)?;
    let out_dir = resolve_out_dir(request.out_dir.as_deref())?;
    let mut warnings: Vec<String> = Vec::new();

    let want_audio = request.want_audio.unwrap_or(true);
    let want_video = request.keep_video.unwrap_or(false);
    let read_screen = request.read_screen.unwrap_or(false);

    if !want_audio && !want_video && !request.want_transcript {
        return Err("Choose at least one thing to keep: audio, video, or a transcript.".to_string());
    }

    ytdlp::emit(&app, ProgressEvent::stage("probing", Some("Reading video info".into())));
    let meta = ytdlp::probe(&url).await?;
    cancel::check()?;

    let job_dir = job_folder(&out_dir, &meta.title, &meta.id)?;

    // The audio call is also what fetches the captions, and Whisper needs the
    // file, so a transcript pulls the audio in even when the MP3 is unwanted.
    // It is deleted at the end in that case rather than never fetched.
    let output = if want_audio || request.want_transcript {
        ytdlp::emit(&app, ProgressEvent::stage("downloading", Some(meta.title.clone())));
        Some(
            ytdlp::download_audio(
                &app,
                DownloadOptions {
                    url: &url,
                    out_dir: &job_dir,
                    video_id: &meta.id,
                    want_subtitles: request.want_transcript,
                    subtitle_langs: &request.transcript_langs,
                    audio_quality: audio_quality_flag(request.audio_quality.as_deref()),
                },
            )
            .await?,
        )
    } else {
        None
    };

    if let Some(output) = &output {
        warnings.extend(output.warnings.iter().cloned());
    }
    cancel::check()?;

    // The video is fetched when the screen is to be read, or simply when the
    // user wants to keep it. Reading is the slow part, so it stays opt-in.
    let (screens, video_path) = if read_screen || want_video {
        screens_from_youtube(&app, &url, &meta.id, &request, &job_dir, &mut warnings).await
    } else {
        (Vec::new(), None)
    };
    cancel::check()?;

    let mut result_transcript: Option<Transcript> = None;

    if let (true, Some(output)) = (request.want_transcript, &output) {
        match build_transcript(&app, output, &request, &meta, &url, &screens, &mut warnings).await {
            Ok(Some(text)) => result_transcript = Some(text),
            Ok(None) => {}
            Err(message) => warnings.push(message),
        }
    }
    cancel::check()?;

    let mut audio_path = None;
    if let Some(output) = &output {
        // Clean up the raw .srt files once their text has been extracted.
        for path in &output.subtitle_paths {
            let _ = std::fs::remove_file(path);
        }

        if want_audio {
            audio_path = Some(output.audio_path.to_string_lossy().into_owned());
        } else {
            let _ = std::fs::remove_file(&output.audio_path);
        }
    }

    // Whatever this job kept, the library gets a picture of it.
    thumb::ensure(&job_dir);

    ytdlp::emit(&app, ProgressEvent::stage("done", None));

    Ok(JobResult {
        title: meta.title,
        folder: job_dir.to_string_lossy().into_owned(),
        audio_path,
        video_path: video_path.map(|p| p.to_string_lossy().into_owned()),
        transcript: result_transcript,
        warnings,
    })
}

/// Captions first (instant, free); whisper.cpp only when there are none.
///
/// Both paths hand back SRT, so timings survive and there is one formatter.
async fn build_transcript(
    app: &AppHandle,
    output: &ytdlp::DownloadOutput,
    request: &JobRequest,
    meta: &VideoMeta,
    url: &str,
    screens: &[Screen],
    warnings: &mut Vec<String>,
) -> Result<Option<Transcript>, String> {
    let mut source = "captions";
    let mut language: Option<String> = None;
    let mut srt: Option<String> = None;

    if let Some(subtitle) = transcript::pick_subtitle(&output.subtitle_paths, &request.transcript_langs) {
        let raw = std::fs::read_to_string(&subtitle)
            .map_err(|e| format!("Could not read subtitle file: {e}"))?;
        if !raw.trim().is_empty() {
            language = language_from_filename(&subtitle);
            srt = Some(raw);
        }
    }

    if srt.is_none() {
        let Some(model_path) = request.whisper_model_path.as_deref().filter(|p| !p.trim().is_empty())
        else {
            warnings.push(
                "No captions on this video. Set a Whisper model path to transcribe the audio instead."
                    .to_string(),
            );
            return Ok(None);
        };

        ytdlp::emit(
            app,
            ProgressEvent::stage("transcribing", Some("No captions found -- running Whisper".into())),
        );

        let lang = request.transcript_langs.first().cloned().unwrap_or_else(|| "auto".into());
        srt = Some(
            transcript::whisper_srt(app, &output.audio_path, &PathBuf::from(model_path), &lang).await?,
        );
        source = "whisper";
        language = Some(lang);
    }

    let cues = transcript::parse_srt(srt.as_deref().unwrap_or_default());
    if cues.is_empty() {
        warnings.push("The transcript came back empty.".to_string());
        return Ok(None);
    }

    let files = write_transcript_outputs(
        &output.audio_path.with_extension("md"),
        &output.audio_path.with_extension("pdf"),
        request.transcript_format.as_deref(),
        &TranscriptHeader {
            title: &meta.title,
            url,
            uploader: meta.uploader.as_deref(),
            duration_seconds: meta.duration_seconds,
            source,
            language: language.as_deref(),
            read_screen: request.read_screen.unwrap_or(false),
        },
        &cues,
        screens,
        &meta.chapters,
    )?;

    Ok(Some(Transcript {
        text: transcript::cues_to_plain_text(&cues),
        source: source.to_string(),
        language,
        file_path: files.markdown_path,
        pdf_path: files.pdf_path,
        markdown: Some(files.markdown),
    }))
}

/// Transcribe a file the user already has. No download, no MP3 conversion --
/// `whisper_srt` feeds it through ffmpeg itself.
#[tauri::command]
pub async fn transcribe_file(app: AppHandle, request: LocalFileRequest) -> Result<JobResult, String> {
    let started = cancel::begin();
    let result = transcribe_file_inner(&app, request).await;
    cancel::finish(result, started)
}

async fn transcribe_file_inner(app: &AppHandle, request: LocalFileRequest) -> Result<JobResult, String> {
    let app = app.clone();
    let path = validate_audio_path(&request.path)?;

    let model_path = request
        .whisper_model_path
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| "Choose a Whisper model under Options first.".to_string())?;

    if binaries::resolve("whisper-cli").is_none() && binaries::resolve("whisper-cpp").is_none() {
        return Err(
            "whisper-cli is not installed. Install it with: brew install whisper-cpp".to_string(),
        );
    }

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string());
    let language = request
        .language
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or("auto")
        .to_string();

    ytdlp::emit(&app, ProgressEvent::stage("transcribing", Some(title.clone())));

    let screens = if request.read_screen.unwrap_or(false) && has_picture(&path) {
        screen::read_screens(&app, &path, &screens_work_dir()).unwrap_or_default()
    } else {
        Vec::new()
    };
    cancel::check()?;

    ytdlp::emit(&app, ProgressEvent::stage("transcribing", Some(title.clone())));

    // The only files this job writes; a stop takes them back.
    cancel::claim(&transcript::transcript_path_for(&path, "md"));
    cancel::claim(&transcript::transcript_path_for(&path, "pdf"));

    let srt = transcript::whisper_srt(&app, &path, &PathBuf::from(model_path), &language).await?;
    let cues = transcript::parse_srt(&srt);
    if cues.is_empty() {
        return Err("The transcript came back empty.".to_string());
    }

    let files = write_transcript_outputs(
        &transcript::transcript_path_for(&path, "md"),
        &transcript::transcript_path_for(&path, "pdf"),
        request.transcript_format.as_deref(),
        &TranscriptHeader {
            title: &title,
            url: &path.to_string_lossy(),
            uploader: None,
            duration_seconds: None,
            source: "whisper",
            language: Some(&language),
            read_screen: request.read_screen.unwrap_or(false),
        },
        &cues,
        &screens,
        &[],
    )?;

    if let Some(folder) = path.parent() {
        thumb::ensure(folder);
    }

    ytdlp::emit(&app, ProgressEvent::stage("done", None));

    Ok(JobResult {
        title,
        folder: path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        audio_path: Some(path.to_string_lossy().into_owned()),
        video_path: None,
        transcript: Some(Transcript {
            text: transcript::cues_to_plain_text(&cues),
            source: "whisper".to_string(),
            language: Some(language),
            file_path: files.markdown_path,
            pdf_path: files.pdf_path,
            markdown: Some(files.markdown),
        }),
        warnings: Vec::new(),
    })
}

fn validate_audio_path(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Choose an audio file first.".to_string());
    }
    let path = PathBuf::from(trimmed);
    if !path.is_file() {
        return Err("That file is no longer there.".to_string());
    }
    if !supported_extension(&path) {
        return Err(
            "That file type is not supported. Try MP3, M4A, WAV, FLAC, OGG, OPUS, AAC, MP4, MOV, MKV or WEBM."
                .to_string(),
        );
    }
    Ok(path)
}

/// Containers worth looking inside for a picture.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "mkv", "webm", "m4v", "avi"];

fn has_picture(path: &std::path::Path) -> bool {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.as_str()))
}

fn screens_work_dir() -> PathBuf {
    std::env::temp_dir().join("yt-mp3-screens")
}

/// Fetch the picture, read it, and throw it away unless it was asked for.
///
/// Failing to read the screen never costs the transcript: the audio is already
/// downloaded by this point, and a warning says what went wrong.
async fn screens_from_youtube(
    app: &AppHandle,
    url: &str,
    video_id: &str,
    request: &JobRequest,
    out_dir: &std::path::Path,
    warnings: &mut Vec<String>,
) -> (Vec<Screen>, Option<PathBuf>) {
    let keep = request.keep_video.unwrap_or(false);
    let work_dir = screens_work_dir();
    let destination = if keep { out_dir } else { work_dir.as_path() };
    cancel::claim(&work_dir);

    let video = match ytdlp::download_video(app, url, destination, video_id).await {
        Ok(path) => path,
        Err(message) => {
            warnings.push(message);
            return (Vec::new(), None);
        }
    };

    let screens = if request.read_screen.unwrap_or(false) {
        ytdlp::emit(
            app,
            ProgressEvent::stage("transcribing", Some("Reading what is on screen".into())),
        );
        match screen::read_screens(app, &video, &work_dir) {
            Ok(screens) => screens,
            Err(message) => {
                warnings.push(message);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    if request.read_screen.unwrap_or(false) && screens.is_empty() {
        warnings.push(
            "Screen reading found nothing to read in this video.".to_string(),
        );
    }

    if keep {
        (screens, Some(video))
    } else {
        let _ = std::fs::remove_file(&video);
        (screens, None)
    }
}

/// Write whichever transcript files the user asked for.
///
/// Both formats render from the same cues, so they always say the same thing.
fn write_transcript_outputs(
    md_path: &std::path::Path,
    pdf_path: &std::path::Path,
    format: Option<&str>,
    header: &TranscriptHeader<'_>,
    cues: &[transcript::Cue],
    screens: &[Screen],
    chapters: &[ytdlp::Chapter],
) -> Result<TranscriptFiles, String> {
    let format = format.map(str::trim).filter(|f| !f.is_empty()).unwrap_or("md");

    // Always built, even when only a PDF is asked for: a queue stitches its
    // combined document out of these.
    let markdown = transcript::cues_to_markdown(header, cues, screens, chapters);
    let mut files = TranscriptFiles { markdown_path: None, pdf_path: None, markdown };

    if format != "pdf" {
        files.markdown_path = Some(
            transcript::write_markdown(md_path, &files.markdown)?
                .to_string_lossy()
                .into_owned(),
        );
    }

    if format != "md" {
        files.pdf_path =
            Some(pdf::save_pdf(pdf_path, header, cues, screens, chapters)?.to_string_lossy().into_owned());
    }

    Ok(files)
}

struct TranscriptFiles {
    markdown_path: Option<String>,
    pdf_path: Option<String>,
    markdown: String,
}

/// Map the UI's choice onto yt-dlp's --audio-quality argument.
fn audio_quality_flag(choice: Option<&str>) -> &'static str {
    match choice {
        Some("320") => "320K", // constant bitrate
        _ => "0",              // LAME VBR V0, ~245 kbps average
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombineSection {
    pub title: String,
    pub source: String,
    pub markdown: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CombineRequest {
    /// In queue order.
    pub sections: Vec<CombineSection>,
    pub out_dir: Option<String>,
    pub transcript_format: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CombineResult {
    pub title: String,
    pub markdown_path: Option<String>,
    pub pdf_path: Option<String>,
}

/// Stitch a finished queue into one document, alongside the per-video files.
#[tauri::command]
pub fn combine_transcripts(request: CombineRequest) -> Result<CombineResult, String> {
    if request.sections.len() < 2 {
        return Err("Nothing to combine: a queue needs at least two transcripts.".to_string());
    }

    let sections: Vec<Section> = request
        .sections
        .into_iter()
        .map(|s| Section { title: s.title, source: s.source, markdown: s.markdown })
        .collect();

    let titles: Vec<String> = sections.iter().map(|s| s.title.clone()).collect();
    let name = combine::combined_name(&titles);
    let out_dir = resolve_out_dir(request.out_dir.as_deref())?;
    let stem = out_dir.join(&name);

    let format = request
        .transcript_format
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .unwrap_or("md");

    let mut result = CombineResult { title: name.clone(), markdown_path: None, pdf_path: None };

    if format != "pdf" {
        let path = transcript::transcript_path_for(&stem, "md");
        let markdown = combine::combined_markdown(&name, &sections);
        result.markdown_path =
            Some(transcript::write_markdown(&path, &markdown)?.to_string_lossy().into_owned());
    }

    if format != "md" {
        let path = transcript::transcript_path_for(&stem, "pdf");
        result.pdf_path =
            Some(pdf::save_combined_pdf(&path, &name, &sections)?.to_string_lossy().into_owned());
    }

    Ok(result)
}

/// Open the library, making it first if it is not there yet.
#[tauri::command]
pub fn open_output_folder(out_dir: Option<String>) -> Result<String, String> {
    let folder = resolve_out_dir(out_dir.as_deref())?;
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not create the folder: {e}"))?;

    reveal_in_file_manager(folder.to_string_lossy().into_owned())?;
    Ok(folder.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Err("That file is no longer there.".to_string());
    }

    #[cfg(target_os = "macos")]
    let command = std::process::Command::new("open").arg("-R").arg(&path).spawn();

    #[cfg(target_os = "windows")]
    let command = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();

    #[cfg(target_os = "linux")]
    let command = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(&path))
        .spawn();

    command.map(|_| ()).map_err(|e| format!("Could not open the folder: {e}"))
}

fn normalize_url(raw: &str) -> Result<String, String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("Paste a YouTube link first.".to_string());
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("That does not look like a link. It should start with https://".to_string());
    }
    Ok(url.to_string())
}

pub(crate) fn resolve_out_dir(requested: Option<&str>) -> Result<PathBuf, String> {
    if let Some(dir) = requested.filter(|d| !d.trim().is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = dirs_home()?;
    Ok(home.join("Documents").join("Tzz Library"))
}

/// One folder per video, inside the library.
///
/// A lecture leaves an MP3, often a video, and a transcript in one or two
/// formats. Ten of them loose in a folder is forty files; in their own folders
/// it is ten lectures, and the video is somewhere you can find it again.
fn job_folder(out_dir: &std::path::Path, title: &str, video_id: &str) -> Result<PathBuf, String> {
    let name = format!("{} [{video_id}]", combine::sanitise(title));
    // 120 bytes keeps the whole path clear of the 255-byte filename limit.
    let trimmed: String = name.chars().take(120).collect();

    let folder = out_dir.join(trimmed.trim());
    // Claimed before it is made, so a stop knows whether it was ours to remove.
    cancel::claim(&folder);
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not create a folder for this video: {e}"))?;
    Ok(folder)
}

fn dirs_home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "Could not find your home folder.".to_string())
}

/// "Clip [abc].sr-Latn.srt" -> "sr-Latn"
fn language_from_filename(path: &std::path::Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy();
    let without_ext = name.strip_suffix(".srt")?;
    without_ext.rsplit_once('.').map(|(_, lang)| lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_audio_and_video_containers() {
        for name in ["a.mp3", "a.M4A", "a.wav", "a.mp4", "a.webm"] {
            assert!(supported_extension(std::path::Path::new(name)), "{name}");
        }
    }

    #[test]
    fn picks_the_largest_model_across_candidate_dirs() {
        let root = std::env::temp_dir().join("yt-mp3-test-models");
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("small.bin"), vec![0u8; 10]).unwrap();
        std::fs::write(b.join("large.bin"), vec![0u8; 500]).unwrap();
        std::fs::write(b.join("notes.txt"), vec![0u8; 9000]).unwrap();

        assert_eq!(largest_model_in(&[a, b]), Some(root.join("b").join("large.bin")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn returns_nothing_when_no_model_is_present() {
        let empty = std::env::temp_dir().join("yt-mp3-test-no-models");
        let _ = std::fs::remove_dir_all(&empty);
        std::fs::create_dir_all(&empty).unwrap();

        assert_eq!(largest_model_in(&[empty.clone()]), None);

        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn names_a_folder_after_the_video_and_keeps_it_openable() {
        let root = std::env::temp_dir().join("yt-mp3-test-folders");
        let _ = std::fs::remove_dir_all(&root);

        let folder = job_folder(&root, "Lumos V2.1 Crash Course (Webflow Framework)", "GPR123").unwrap();

        assert_eq!(
            folder,
            root.join("Lumos V2.1 Crash Course (Webflow Framework) [GPR123]"),
        );
        assert!(folder.is_dir(), "the folder is created, not just named");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_slash_in_the_title_does_not_become_a_folder() {
        let root = std::env::temp_dir().join("yt-mp3-test-slash");
        let _ = std::fs::remove_dir_all(&root);

        let folder = job_folder(&root, "A/B testing: what works", "x1").unwrap();

        assert_eq!(folder.parent(), Some(root.as_path()), "still one level deep");
        assert!(folder.file_name().unwrap().to_string_lossy().starts_with("A-B testing"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_everything_else() {
        for name in ["notes.txt", "archive.zip", "noextension"] {
            assert!(!supported_extension(std::path::Path::new(name)), "{name}");
        }
    }
}
