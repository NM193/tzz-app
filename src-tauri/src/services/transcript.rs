//! Turning a downloaded video into readable text.
//!
//! Both sources produce SRT, so there is exactly one formatting path:
//!   1. YouTube captions (free, instant) -- reliable for English, often missing for Serbian.
//!   2. whisper.cpp on the MP3 (slower, local, works for any language).
//!
//! SRT keeps the timings, which is the whole point: the output carries
//! timestamps so a passage can be traced back to a spot in the video.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::binaries;
use super::ytdlp::{emit, Chapter, ProgressEvent};

/// How much speech goes into one timestamped paragraph.
const PARAGRAPH_SECONDS: u64 = 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// Plain running text, used for the in-app preview and the clipboard.
    pub text: String,
    /// "captions" | "whisper"
    pub source: String,
    pub language: Option<String>,
    /// Path to the saved .md file, when one was asked for.
    pub file_path: Option<String>,
    /// Path to the saved .pdf file, when one was asked for.
    pub pdf_path: Option<String>,
    /// The timestamped document itself, so a queue can stitch several together
    /// without re-reading files or re-running Whisper.
    pub markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: u64,
    pub text: String,
}

/// Everything the markdown header needs, so a file read months later is
/// self-explanatory without the app.
pub struct TranscriptHeader<'a> {
    pub title: &'a str,
    pub url: &'a str,
    pub uploader: Option<&'a str>,
    pub duration_seconds: Option<f64>,
    pub source: &'a str,
    pub language: Option<&'a str>,
    /// Whether reading the screen was asked for. Recorded either way, so a
    /// document with no screens cannot be confused with a job that never
    /// looked.
    pub read_screen: bool,
}

/// Pick the best subtitle file for the requested languages.
///
/// Filenames look like `Title [id].en.srt` or `Title [id].sr-Latn.srt`, so we
/// match on the language segment and honour the caller's preference order.
pub fn pick_subtitle(paths: &[PathBuf], preferred_langs: &[String]) -> Option<PathBuf> {
    // A preferred language in the video's own tongue: the best of both.
    for lang in preferred_langs {
        if let Some(path) = find_track(paths, &format!("{lang}-orig")) {
            return Some(path);
        }
    }

    // Otherwise the original beats a preferred language, because anything
    // else YouTube offers is a machine translation of a machine transcript.
    // On an English Webflow course the Serbian track turned Montserrat into
    // "Монзерат" and Impact into "утицај" -- the terms being taught, lost.
    if let Some(path) = paths.iter().find(|path| track_name(path).contains("-orig.srt")) {
        return Some(path.clone());
    }

    for lang in preferred_langs {
        if let Some(path) = find_track(paths, lang) {
            return Some(path);
        }
    }

    paths.first().cloned()
}

fn find_track(paths: &[PathBuf], lang: &str) -> Option<PathBuf> {
    let needle = format!(".{}", lang.to_lowercase());
    paths
        .iter()
        .find(|path| {
            let name = track_name(path);
            // ".en.srt" or ".en-US.srt" -- but not ".english.srt"
            name.contains(&format!("{needle}.srt")) || name.contains(&format!("{needle}-"))
        })
        .cloned()
}

fn track_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Parse SRT into timed cues, with YouTube's rolling duplicates collapsed.
pub fn parse_srt(srt: &str) -> Vec<Cue> {
    let normalized = srt.replace("\r\n", "\n");
    let mut cues: Vec<Cue> = Vec::new();

    for block in normalized.split("\n\n") {
        let mut start_ms: Option<u64> = None;
        let mut text_parts: Vec<String> = Vec::new();

        for raw_line in block.lines() {
            let line = raw_line.trim();
            if line.is_empty() {
                continue;
            }
            if line.contains("-->") {
                start_ms = line.split("-->").next().and_then(|s| parse_timecode(s.trim()));
                continue;
            }
            // The cue number, only when it appears before the timing line.
            if start_ms.is_none() && line.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let cleaned = strip_tags(line);
            if !cleaned.is_empty() {
                text_parts.push(cleaned);
            }
        }

        if let Some(start_ms) = start_ms {
            if !text_parts.is_empty() {
                push_cue(&mut cues, Cue { start_ms, text: text_parts.join(" ") });
            }
        }
    }

    cues
}

/// Auto-captions repeat the previous line and append a few words, so each cue
/// is often a superset of the one before it. Replace instead of accumulating.
/// Shortest repeat we are willing to call an artefact rather than speech.
///
/// One word is too eager: a cue ending in "the" followed by one starting with
/// "the" is ordinary. Two or more words repeating across a cue boundary is
/// the scrolling caption, not the speaker.
const MIN_OVERLAP_WORDS: usize = 2;

fn push_cue(cues: &mut Vec<Cue>, cue: Cue) {
    let Some(previous) = cues.last_mut() else {
        cues.push(cue);
        return;
    };

    if cue.text == previous.text {
        return;
    }

    // The cue grew out of the previous one: keep the longer version.
    if cue.text.starts_with(previous.text.as_str()) {
        previous.text = cue.text;
        return;
    }

    let trimmed = strip_overlap(&previous.text, &cue.text);
    if trimmed.is_empty() {
        return;
    }
    cues.push(Cue { start_ms: cue.start_ms, text: trimmed });
}

/// Drop the opening words of `next` that merely repeat the tail of `previous`.
///
/// YouTube's auto-captions scroll: every cue restates the last line of the one
/// before it. Joined verbatim, that doubles the whole transcript. Matching is
/// done on words so a repeat is never cut mid-word, and the longest overlap
/// wins so a two-line carry-over is removed whole.
fn strip_overlap(previous: &str, next: &str) -> String {
    let prev_words: Vec<&str> = previous.split_whitespace().collect();
    let next_words: Vec<&str> = next.split_whitespace().collect();
    let longest = prev_words.len().min(next_words.len());

    for overlap in (MIN_OVERLAP_WORDS..=longest).rev() {
        if prev_words[prev_words.len() - overlap..] == next_words[..overlap] {
            return next_words[overlap..].join(" ");
        }
    }

    next.to_string()
}

/// "00:01:23,456" -> milliseconds
fn parse_timecode(raw: &str) -> Option<u64> {
    let raw = raw.replace(',', ".");
    let mut parts = raw.split(':');
    let hours: u64 = parts.next()?.trim().parse().ok()?;
    let minutes: u64 = parts.next()?.trim().parse().ok()?;
    let seconds: f64 = parts.next()?.trim().parse().ok()?;
    Some((hours * 3600 + minutes * 60) * 1000 + (seconds * 1000.0) as u64)
}

fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut depth_angle = 0usize;
    let mut depth_brace = 0usize;

    for ch in line.chars() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle = depth_angle.saturating_sub(1),
            '{' => depth_brace += 1,
            '}' => depth_brace = depth_brace.saturating_sub(1),
            _ if depth_angle == 0 && depth_brace == 0 => out.push(ch),
            _ => {}
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Running text with no timings, for the preview pane and the clipboard.
pub fn cues_to_plain_text(cues: &[Cue]) -> String {
    cues.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" ")
}

/// A markdown file with a metadata header and one timestamped paragraph per
/// minute. The header exists so the file stands on its own when it is handed
/// to something else later.
pub fn cues_to_markdown(
    header: &TranscriptHeader<'_>,
    cues: &[Cue],
    screens: &[Screen],
    chapters: &[Chapter],
) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", header.title));
    if let Some(uploader) = header.uploader {
        out.push_str(&format!("- Channel: {uploader}\n"));
    }
    out.push_str(&format!("- Source: {}\n", header.url));
    if let Some(duration) = header.duration_seconds {
        out.push_str(&format!("- Duration: {}\n", format_clock((duration * 1000.0) as u64)));
    }
    out.push_str(&format!(
        "- Transcript from: {}{}\n",
        header.source,
        header.language.map(|l| format!(" ({l})")).unwrap_or_default()
    ));
    // Says plainly whether the picture was read, so a document with no screens
    // cannot be mistaken for a video that simply had nothing on screen.
    out.push_str(&match (header.read_screen, screens.len()) {
        (false, _) => "- Screens: not read (the option was off)\n".to_string(),
        (true, 0) => "- Screens: reading was on, but nothing was found\n".to_string(),
        (true, count) => format!("- Screens read: {count}\n"),
    });
    out.push_str("\n---\n\n");

    if cues.is_empty() {
        out.push_str("_No speech was found._\n");
        return out;
    }

    if !chapters.is_empty() {
        out.push_str("## Contents\n\n");
        for (index, chapter) in chapters.iter().enumerate() {
            out.push_str(&format!(
                "{}. [{}](#{}) — {}\n",
                index + 1,
                chapter.title,
                heading_slug(index + 1, &chapter.title),
                format_clock(chapter_start_ms(chapter)),
            ));
        }
        out.push_str("\n---\n\n");
    }

    let boundaries: Vec<u64> = chapters.iter().map(chapter_start_ms).collect();
    let paragraphs = group_into_paragraphs(cues, &boundaries);
    let mut next_chapter = 0;

    for (index, paragraph) in paragraphs.iter().enumerate() {
        // A screen belongs to exactly one paragraph: the one it falls inside.
        // Paragraphs no longer last a fixed minute -- a chapter can open one
        // early -- so the end is wherever the next paragraph begins.
        let ends_at = paragraphs.get(index + 1).map_or(u64::MAX, |next| next.start_ms);

        // A chapter heading lands before the paragraph that opens it.
        while next_chapter < chapters.len()
            && chapter_start_ms(&chapters[next_chapter]) <= paragraph.start_ms
        {
            let chapter = &chapters[next_chapter];
            out.push_str(&format!(
                "## {}. {}\n\n*{}*\n\n",
                next_chapter + 1,
                chapter.title,
                format_clock(chapter_start_ms(chapter)),
            ));
            next_chapter += 1;
        }

        out.push_str(&format!(
            "**[{}]** {}\n\n",
            format_clock(paragraph.start_ms),
            paragraph.text
        ));

        for screen in screens_between(screens, paragraph.start_ms, ends_at) {
            out.push_str(&format!(
                "> **Screen {}** -- {}\n\n",
                format_clock(screen.at_ms),
                screen.lines.join(" · ")
            ));
        }
    }

    out
}

pub fn chapter_start_ms(chapter: &Chapter) -> u64 {
    (chapter.start_seconds * 1000.0) as u64
}

/// The anchor a markdown viewer gives "## 3. Figma File", so the contents can
/// link into the body.
fn heading_slug(number: usize, title: &str) -> String {
    let mut slug = format!("{number}-{}", title.to_lowercase());
    slug = slug
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug.trim_matches('-').to_string()
}

fn screens_between(screens: &[Screen], from_ms: u64, to_ms: u64) -> impl Iterator<Item = &Screen> {
    screens.iter().filter(move |screen| screen.at_ms >= from_ms && screen.at_ms < to_ms)
}

/// What the screen showed at a moment, once the repeats are stripped out.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct Screen {
    pub at_ms: u64,
    pub lines: Vec<String>,
}

/// One timestamped paragraph of speech.
pub struct Paragraph {
    pub start_ms: u64,
    pub text: String,
}

/// Bucket cues into one paragraph per minute.
///
/// Markdown and PDF both render from this, so the two outputs can never drift
/// apart in how the speech is broken up.
/// Bucket cues into one paragraph per minute, breaking early at each boundary.
///
/// WHY the boundaries: chapters start mid-minute, and this course puts 43 of
/// them in 64 minutes. Without a break the speech from 0:00 to 0:35 lands in
/// the same paragraph as what follows, so it gets filed under the next chapter
/// and the first one reads as empty.
pub fn group_into_paragraphs(cues: &[Cue], boundaries: &[u64]) -> Vec<Paragraph> {
    let bucket_ms = PARAGRAPH_SECONDS * 1000;
    let mut out: Vec<Paragraph> = Vec::new();
    let mut current: Option<(u64, usize)> = None;
    let mut pending: Vec<&str> = Vec::new();

    for cue in cues {
        let bucket = cue.start_ms / bucket_ms;
        let section = boundaries.iter().filter(|&&start| start <= cue.start_ms).count();

        if current != Some((bucket, section)) {
            if let Some((bucket, section)) = current {
                out.push(paragraph_of(bucket * bucket_ms, section, boundaries, &pending));
                pending.clear();
            }
            current = Some((bucket, section));
        }
        pending.push(&cue.text);
    }

    if let Some((bucket, section)) = current {
        out.push(paragraph_of(bucket * bucket_ms, section, boundaries, &pending));
    }

    out
}

/// A paragraph opened by a boundary is timed from the boundary, not the minute.
fn paragraph_of(bucket_start: u64, section: usize, boundaries: &[u64], lines: &[&str]) -> Paragraph {
    let start_ms = match section.checked_sub(1).and_then(|i| boundaries.get(i)) {
        Some(&boundary) if boundary > bucket_start => boundary,
        _ => bucket_start,
    };
    Paragraph { start_ms, text: lines.join(" ") }
}

/// mm:ss below an hour, h:mm:ss above it.
pub(super) fn format_clock(ms: u64) -> String {
    let total = ms / 1000;
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Transcribe an MP3 locally with whisper.cpp, returning raw SRT.
///
/// whisper.cpp only accepts 16 kHz mono PCM, so ffmpeg converts first into a
/// temp file we clean up afterwards.
pub async fn whisper_srt(
    app: &AppHandle,
    audio_path: &Path,
    model_path: &Path,
    language: &str,
) -> Result<String, String> {
    if !model_path.is_file() {
        return Err(format!(
            "Whisper model not found at {}. Download one from huggingface.co/ggerganov/whisper.cpp",
            model_path.display()
        ));
    }

    let whisper = binaries::resolve("whisper-cli")
        .or_else(|| binaries::resolve("whisper-cpp"))
        .or_else(|| binaries::resolve("main"))
        .ok_or_else(|| {
            "whisper-cli not found. Install it with: brew install whisper-cpp".to_string()
        })?;
    let ffmpeg = binaries::require("ffmpeg")?;

    let work_dir = std::env::temp_dir().join("yt-mp3-whisper");
    std::fs::create_dir_all(&work_dir)
        .map_err(|e| format!("Could not create temp folder: {e}"))?;

    let stem = audio_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".to_string());
    let wav_path = work_dir.join(format!("{stem}.wav"));
    let out_base = work_dir.join(&stem);

    // A three-hour file takes ffmpeg a couple of minutes; say so rather than
    // leaving the meter frozen before whisper even starts.
    emit(
        app,
        ProgressEvent::stage("converting", Some("Preparing audio for Whisper".to_string())),
    );

    let convert = Command::new(ffmpeg)
        .args(["-y", "-i"])
        .arg(audio_path)
        .args(["-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le"])
        .arg(&wav_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| format!("Could not run ffmpeg: {e}"))?;

    if !convert.success() {
        return Err("ffmpeg could not prepare the audio for transcription.".to_string());
    }

    // -osrt keeps the timings; -pp makes whisper report how far it has got,
    // which is the only progress signal a long file gives us. Transcribed
    // segments go to stdout and are dropped -- the SRT file is the real output.
    let mut child = Command::new(whisper)
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(&wav_path)
        .args(["-l", language, "-osrt", "-pp"])
        .arg("-of")
        .arg(&out_base)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not run whisper: {e}"))?;

    let stderr = child.stderr.take().expect("stderr piped");
    let mut lines = BufReader::new(stderr).lines();

    // Keep the last lines so a failure can report something actionable.
    let mut log: Vec<String> = Vec::new();

    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(percent) = parse_whisper_progress(&line) {
            emit(
                app,
                ProgressEvent {
                    stage: "transcribing".to_string(),
                    percent: Some(percent),
                    detail: None,
                },
            );
            continue;
        }
        log.push(line);
        if log.len() > 40 {
            log.remove(0);
        }
    }

    child
        .wait()
        .await
        .map_err(|e| format!("Whisper did not finish cleanly: {e}"))?;

    let srt_path = out_base.with_extension("srt");
    let srt = std::fs::read_to_string(&srt_path).map_err(|_| {
        let tail: Vec<&str> = log.iter().rev().take(4).map(String::as_str).collect();
        format!("Whisper produced no transcript. {}", tail.join(" "))
    })?;

    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(&srt_path);

    Ok(srt)
}

/// `whisper_print_progress_callback: progress =  35%` -> `35.0`
fn parse_whisper_progress(line: &str) -> Option<f32> {
    let rest = line.trim().strip_prefix("whisper_print_progress_callback:")?;
    let value = rest.trim().strip_prefix("progress")?.trim().strip_prefix('=')?;
    value.trim().trim_end_matches('%').parse::<f32>().ok()
}

/// Read a transcript document back into cues, one per paragraph.
///
/// The format is ours (`**[mm:ss]** text`), so this is the inverse of
/// `cues_to_markdown`. It exists so a combined document can be rendered
/// through exactly the same path as a single one.
pub fn parse_markdown_paragraphs(markdown: &str) -> Vec<Cue> {
    markdown
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("**[")?;
            let (clock, text) = rest.split_once("]**")?;
            Some(Cue { start_ms: parse_clock(clock)?, text: text.trim().to_string() })
        })
        .filter(|cue| !cue.text.is_empty())
        .collect()
}

/// "mm:ss" or "h:mm:ss" -> milliseconds. The inverse of `format_clock`.
fn parse_clock(clock: &str) -> Option<u64> {
    let mut seconds: u64 = 0;
    for part in clock.split(':') {
        seconds = seconds * 60 + part.parse::<u64>().ok()?;
    }
    Some(seconds * 1000)
}

/// Where a local file's transcript should go.
///
/// A downloaded MP3 owns its `.md` neighbour, but a file the user already had
/// may not: `interview.mp3` must not clobber an `interview.md` they wrote
/// themselves. When the plain name is taken we step aside.
pub fn transcript_path_for(audio_path: &Path, extension: &str) -> PathBuf {
    let plain = audio_path.with_extension(extension);
    if !plain.exists() {
        return plain;
    }
    let stem = audio_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".to_string());
    audio_path.with_file_name(format!("{stem}.transcript.{extension}"))
}

pub fn write_markdown(md_path: &Path, markdown: &str) -> Result<PathBuf, String> {
    std::fs::write(md_path, markdown)
        .map_err(|e| format!("Could not save the transcript: {e}"))?;
    Ok(md_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_the_scrolling_overlap_youtube_actually_emits() {
        // Each cue restates the previous cue's last line, which is what makes
        // the transcript come out at double length.
        let srt = "1\n00:00:01,000 --> 00:00:03,000\ntune these models so that's why I think\nmost of the existing context\n\n\
                   2\n00:00:03,000 --> 00:00:05,000\nmost of the existing context\nstill apply we did drop some\n";

        let cues = parse_srt(srt);
        let joined = cues_to_plain_text(&cues);

        assert_eq!(joined.matches("most of the existing context").count(), 1, "{joined}");
        assert!(joined.ends_with("still apply we did drop some"), "{joined}");
    }

    #[test]
    fn leaves_a_genuine_short_repeat_alone() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nit is the\n\n2\n00:00:02,000 --> 00:00:03,000\nthe second point\n";
        let joined = cues_to_plain_text(&parse_srt(srt));
        assert_eq!(joined, "it is the the second point", "one repeated word is not the artefact");
    }

    #[test]
    fn a_screen_is_printed_once_even_when_chapters_split_the_minute() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None, read_screen: false,
        };
        let cues = vec![
            Cue { start_ms: 2_000, text: "intro".into() },
            Cue { start_ms: 40_000, text: "resources".into() },
            Cue { start_ms: 70_000, text: "next minute".into() },
        ];
        let chapters = vec![chapter("Intro", 0.0), chapter("Starting Resources", 35.0)];
        let screens = vec![Screen { at_ms: 50_000, lines: vec!["Style Guide".into()] }];

        let md = cues_to_markdown(&header, &cues, &screens, &chapters);

        assert_eq!(md.matches("Screen 00:50").count(), 1, "once and only once: {md}");
    }

    #[test]
    fn a_chapter_starting_mid_minute_keeps_its_own_speech() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None, read_screen: false,
        };
        let cues = vec![
            Cue { start_ms: 2_000, text: "belongs to the intro".into() },
            Cue { start_ms: 40_000, text: "belongs to resources".into() },
        ];
        let chapters = vec![chapter("Intro", 0.0), chapter("Starting Resources", 35.0)];

        let md = cues_to_markdown(&header, &cues, &[], &chapters);
        let body: Vec<&str> = md
            .lines()
            .filter(|l| l.starts_with("## ") || l.starts_with("**["))
            .collect();

        assert_eq!(body[0], "## Contents");
        assert_eq!(body[1], "## 1. Intro");
        assert_eq!(body[2], "**[00:00]** belongs to the intro", "the intro is not left empty");
        assert_eq!(body[3], "## 2. Starting Resources");
        assert_eq!(body[4], "**[00:35]** belongs to resources");
    }

    #[test]
    fn puts_each_screen_under_the_minute_it_appeared_in() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None,
                    read_screen: false,
        };
        let cues = vec![
            Cue { start_ms: 0, text: "first".into() },
            Cue { start_ms: 61_000, text: "second".into() },
        ];
        let screens = vec![
            Screen { at_ms: 30_000, lines: vec!["Global Styles".into()] },
            Screen { at_ms: 70_000, lines: vec!["Visual Video".into(), "Autoplay".into()] },
        ];

        let md = cues_to_markdown(&header, &cues, &screens, &[]);
        let body: Vec<&str> = md.lines().filter(|l| l.starts_with("**[") || l.starts_with("> ")).collect();

        assert_eq!(body[0], "**[00:00]** first");
        assert_eq!(body[1], "> **Screen 00:30** -- Global Styles");
        assert_eq!(body[2], "**[01:00]** second");
        assert_eq!(body[3], "> **Screen 01:10** -- Visual Video · Autoplay");
    }

    #[test]
    fn without_screens_the_document_is_unchanged() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None,
                    read_screen: false,
        };
        let cues = vec![Cue { start_ms: 0, text: "only speech".into() }];

        let md = cues_to_markdown(&header, &cues, &[], &[]);

        assert!(!md.contains("> **Screen"), "no screen entries in the body: {md}");
        assert!(md.contains("- Screens: not read"), "but the header says so: {md}");
        assert!(md.contains("**[00:00]** only speech"));
    }

    #[test]
    fn the_originals_language_beats_a_machine_translation() {
        let tracks = vec![
            PathBuf::from("Course [id].sr.srt"),
            PathBuf::from("Course [id].en.srt"),
            PathBuf::from("Course [id].en-orig.srt"),
        ];
        let prefer_serbian = vec!["sr".to_string(), "en".to_string()];

        assert_eq!(
            pick_subtitle(&tracks, &prefer_serbian),
            Some(PathBuf::from("Course [id].en-orig.srt")),
        );
    }

    #[test]
    fn a_preferred_language_wins_when_it_is_the_original() {
        let tracks = vec![
            PathBuf::from("Predavanje [id].sr-orig.srt"),
            PathBuf::from("Predavanje [id].en.srt"),
        ];
        assert_eq!(
            pick_subtitle(&tracks, &["sr".to_string(), "en".to_string()]),
            Some(PathBuf::from("Predavanje [id].sr-orig.srt")),
        );
    }

    #[test]
    fn falls_back_to_a_translation_when_there_is_no_original() {
        let tracks = vec![PathBuf::from("Clip [id].sr.srt"), PathBuf::from("Clip [id].de.srt")];
        assert_eq!(
            pick_subtitle(&tracks, &["sr".to_string()]),
            Some(PathBuf::from("Clip [id].sr.srt")),
        );
    }

    fn chapter(title: &str, seconds: f64) -> Chapter {
        Chapter { title: title.to_string(), start_seconds: seconds }
    }

    #[test]
    fn builds_a_contents_list_that_links_into_the_body() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None, read_screen: false,
        };
        let cues = vec![
            Cue { start_ms: 0, text: "opening".into() },
            Cue { start_ms: 130_000, text: "later".into() },
        ];
        let chapters = vec![chapter("Intro", 0.0), chapter("Figma File", 130.0)];

        let md = cues_to_markdown(&header, &cues, &[], &chapters);

        assert!(md.contains("## Contents"), "{md}");
        assert!(md.contains("1. [Intro](#1-intro) — 00:00"), "{md}");
        assert!(md.contains("2. [Figma File](#2-figma-file) — 02:10"), "{md}");
        assert!(md.contains("## 1. Intro"), "{md}");
        assert!(md.contains("## 2. Figma File"), "{md}");
    }

    #[test]
    fn a_video_without_chapters_gets_no_contents() {
        let header = TranscriptHeader {
            title: "T", url: "u", uploader: None,
            duration_seconds: None, source: "captions", language: None, read_screen: false,
        };
        let cues = vec![Cue { start_ms: 0, text: "just speech".into() }];

        let md = cues_to_markdown(&header, &cues, &[], &[]);

        assert!(!md.contains("Contents"), "{md}");
        assert!(!md.contains("## 1."), "{md}");
    }

    #[test]
    fn slugs_match_what_a_markdown_viewer_makes() {
        assert_eq!(heading_slug(1, "Intro"), "1-intro");
        assert_eq!(heading_slug(12, "Component First Mindset"), "12-component-first-mindset");
        assert_eq!(heading_slug(3, "Grid & Flex: basics"), "3-grid-flex-basics");
    }

    #[test]
    fn groups_cues_into_one_paragraph_per_minute() {
        let cues = vec![
            Cue { start_ms: 0, text: "one".into() },
            Cue { start_ms: 30_000, text: "two".into() },
            Cue { start_ms: 61_000, text: "three".into() },
        ];

        let paragraphs = group_into_paragraphs(&cues, &[]);

        assert_eq!(paragraphs.len(), 2);
        assert_eq!(paragraphs[0].start_ms, 0);
        assert_eq!(paragraphs[0].text, "one two");
        assert_eq!(paragraphs[1].start_ms, 60_000);
        assert_eq!(paragraphs[1].text, "three");
    }

    #[test]
    fn groups_nothing_when_there_are_no_cues() {
        assert!(group_into_paragraphs(&[], &[]).is_empty());
    }

    #[test]
    fn reads_whispers_progress_lines() {
        assert_eq!(
            parse_whisper_progress("whisper_print_progress_callback: progress =  35%"),
            Some(35.0)
        );
        assert_eq!(
            parse_whisper_progress("whisper_print_progress_callback: progress = 100%"),
            Some(100.0)
        );
    }

    #[test]
    fn ignores_whispers_other_chatter() {
        for line in [
            "whisper_init_from_file_with_params_no_state: loading model",
            "main: processing 'x.wav' (1234 samples)",
            "",
        ] {
            assert_eq!(parse_whisper_progress(line), None, "{line}");
        }
    }

    #[test]
    fn transcript_path_uses_the_plain_name_when_free() {
        let dir = std::env::temp_dir().join("yt-mp3-test-free");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("interview.mp3");

        assert_eq!(transcript_path_for(&audio, "md"), dir.join("interview.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn transcript_path_steps_aside_rather_than_overwrite() {
        let dir = std::env::temp_dir().join("yt-mp3-test-taken");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let audio = dir.join("interview.mp3");
        std::fs::write(dir.join("interview.md"), "my own notes").unwrap();

        assert_eq!(transcript_path_for(&audio, "md"), dir.join("interview.transcript.md"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_timings_and_strips_tags() {
        let srt = "1\n00:00:01,000 --> 00:00:03,000\n<i>Hello</i> there\n\n2\n00:01:05,500 --> 00:01:07,000\nSecond bit\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0], Cue { start_ms: 1000, text: "Hello there".into() });
        assert_eq!(cues[1].start_ms, 65_500);
    }

    #[test]
    fn collapses_rolling_auto_caption_duplicates() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nhello world\n\n2\n00:00:02,000 --> 00:00:03,000\nhello world this is\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "hello world this is");
    }

    #[test]
    fn groups_cues_into_timestamped_paragraphs() {
        let cues = vec![
            Cue { start_ms: 0, text: "one".into() },
            Cue { start_ms: 30_000, text: "two".into() },
            Cue { start_ms: 61_000, text: "three".into() },
        ];
        let header = TranscriptHeader {
            title: "Clip",
            url: "https://example.com",
            uploader: None,
            duration_seconds: None,
            source: "captions",
            language: Some("en"),
                    read_screen: false,
        };
        let md = cues_to_markdown(&header, &cues, &[], &[]);
        assert!(md.contains("**[00:00]** one two"));
        assert!(md.contains("**[01:00]** three"));
    }

    #[test]
    fn prefers_the_first_requested_language() {
        let paths = vec![
            PathBuf::from("Clip [abc].en.srt"),
            PathBuf::from("Clip [abc].sr.srt"),
        ];
        let langs = vec!["sr".to_string(), "en".to_string()];
        assert_eq!(
            pick_subtitle(&paths, &langs).unwrap(),
            PathBuf::from("Clip [abc].sr.srt")
        );
    }
}

#[cfg(test)]
mod roundtrip_tests {
    use super::*;

    #[test]
    fn markdown_parses_back_into_the_paragraphs_it_was_built_from() {
        let cues = vec![
            Cue { start_ms: 0, text: "first minute".into() },
            Cue { start_ms: 61_000, text: "second minute".into() },
            Cue { start_ms: 3_600_000, text: "an hour in".into() },
        ];
        let header = TranscriptHeader {
            title: "T",
            url: "u",
            uploader: None,
            duration_seconds: None,
            source: "whisper",
            language: None,
            read_screen: false,
        };

        let parsed = parse_markdown_paragraphs(&cues_to_markdown(&header, &cues, &[], &[]));

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].text, "first minute");
        assert_eq!(parsed[1].start_ms, 60_000);
        assert_eq!(parsed[2].start_ms, 3_600_000);
        assert_eq!(parsed[2].text, "an hour in");
    }

    #[test]
    fn ignores_the_header_and_the_rule() {
        let md = "# Title\n\n- Source: x\n\n---\n\n**[00:10]** only this\n";
        let parsed = parse_markdown_paragraphs(md);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].text, "only this");
    }
}
