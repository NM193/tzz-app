//! Rendering a transcript as a PDF.
//!
//! WHY this is not just the markdown converted: a PDF is meant to be read, so
//! it wants real typography -- proportional text, a bold timestamp per
//! paragraph, page numbers -- not the source syntax on the page. Both outputs
//! are built from the same `group_into_paragraphs`, so they cannot drift apart
//! in how the speech is broken up.
//!
//! Fonts come from the system rather than a bundled file. Serbian needs
//! c-caron, c-acute, s-caron, z-caron and d-stroke, which the built-in PDF
//! fonts do not carry, and macOS already ships faces that do.

use std::path::{Path, PathBuf};

use genpdf::style::{Color, Style};
use genpdf::{elements, fonts, Alignment, Element, Margins, Mm, SimplePageDecorator};

use super::combine::Section;
use super::transcript::{self, Cue, Screen, TranscriptHeader};
use super::ytdlp::Chapter;

const FONT_DIR: &str = "/System/Library/Fonts/Supplemental";

/// Tried in order. Each entry is regular, bold, italic, bold-italic.
const FONT_FAMILIES: &[[&str; 4]] = &[
    ["Arial.ttf", "Arial Bold.ttf", "Arial Italic.ttf", "Arial Bold Italic.ttf"],
    [
        "Times New Roman.ttf",
        "Times New Roman Bold.ttf",
        "Times New Roman Italic.ttf",
        "Times New Roman Bold Italic.ttf",
    ],
];

const MUTED: Color = Color::Rgb(110, 110, 110);

/// Write the transcript to `pdf_path`.
pub fn save_pdf(
    pdf_path: &Path,
    header: &TranscriptHeader<'_>,
    cues: &[Cue],
    screens: &[Screen],
    chapters: &[Chapter],
) -> Result<PathBuf, String> {
    let mut doc = genpdf::Document::new(load_font_family()?);
    doc.set_title(header.title);
    doc.set_font_size(11);
    doc.set_line_spacing(1.3);

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(Margins::trbl(
        Mm::from(20.0),
        Mm::from(18.0),
        Mm::from(20.0),
        Mm::from(18.0),
    ));
    decorator.set_header(page_header);
    doc.set_page_decorator(decorator);

    doc.push(
        elements::Paragraph::new(header.title)
            .styled(Style::new().bold().with_font_size(18)),
    );
    doc.push(elements::Break::new(0.6));

    for line in meta_lines(header) {
        doc.push(
            elements::Paragraph::new(line)
                .styled(Style::new().with_font_size(9).with_color(MUTED)),
        );
    }
    doc.push(elements::Break::new(1.2));

    if cues.is_empty() {
        doc.push(
            elements::Paragraph::new("No speech was found.")
                .styled(Style::new().italic().with_color(MUTED)),
        );
    }

    if !chapters.is_empty() {
        doc.push(elements::Paragraph::new("Contents").styled(Style::new().bold().with_font_size(13)));
        doc.push(elements::Break::new(0.5));

        for (index, chapter) in chapters.iter().enumerate() {
            doc.push(
                elements::Paragraph::new(format!(
                    "{}.  {}   {}",
                    index + 1,
                    chapter.title,
                    transcript::format_clock(transcript::chapter_start_ms(chapter)),
                ))
                .styled(Style::new().with_font_size(10)),
            );
        }
        doc.push(elements::Break::new(1.4));
    }

    push_paragraphs(&mut doc, cues, screens, chapters);

    doc.render_to_file(pdf_path)
        .map_err(|e| format!("Could not write the PDF: {e}"))?;

    Ok(pdf_path.to_path_buf())
}

/// The same document, one chapter per video.
pub fn save_combined_pdf(
    pdf_path: &Path,
    name: &str,
    sections: &[Section],
) -> Result<PathBuf, String> {
    let mut doc = genpdf::Document::new(load_font_family()?);
    doc.set_title(name);
    doc.set_font_size(11);
    doc.set_line_spacing(1.3);

    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(Margins::trbl(
        Mm::from(20.0),
        Mm::from(18.0),
        Mm::from(20.0),
        Mm::from(18.0),
    ));
    decorator.set_header(page_header);
    doc.set_page_decorator(decorator);

    doc.push(elements::Paragraph::new(name).styled(Style::new().bold().with_font_size(18)));
    doc.push(elements::Break::new(0.6));
    doc.push(
        elements::Paragraph::new(format!("Combined from {} videos", sections.len()))
            .styled(Style::new().with_font_size(9).with_color(MUTED)),
    );
    doc.push(elements::Break::new(1.4));

    for (index, section) in sections.iter().enumerate() {
        doc.push(
            elements::Paragraph::new(format!("{}. {}", index + 1, section.title))
                .styled(Style::new().bold().with_font_size(14)),
        );
        doc.push(elements::Break::new(0.4));
        doc.push(
            elements::Paragraph::new(section.source.clone())
                .styled(Style::new().with_font_size(9).with_color(MUTED)),
        );
        doc.push(elements::Break::new(0.9));

        push_paragraphs(&mut doc, &transcript::parse_markdown_paragraphs(&section.markdown), &[], &[]);
        doc.push(elements::Break::new(1.2));
    }

    doc.render_to_file(pdf_path)
        .map_err(|e| format!("Could not write the PDF: {e}"))?;

    Ok(pdf_path.to_path_buf())
}

/// A bold timestamp, then the speech. Shared so both documents read alike.
fn push_paragraphs(
    doc: &mut genpdf::Document,
    cues: &[Cue],
    screens: &[Screen],
    chapters: &[Chapter],
) {
    let boundaries: Vec<u64> = chapters.iter().map(transcript::chapter_start_ms).collect();
    let paragraphs = transcript::group_into_paragraphs(cues, &boundaries);
    let mut next_chapter = 0;

    for (index, paragraph) in paragraphs.iter().enumerate() {
        // One paragraph owns each screen; a chapter can end one early.
        let ends_at = paragraphs.get(index + 1).map_or(u64::MAX, |next| next.start_ms);
        while next_chapter < chapters.len()
            && transcript::chapter_start_ms(&chapters[next_chapter]) <= paragraph.start_ms
        {
            let chapter = &chapters[next_chapter];
            doc.push(elements::Break::new(0.6));
            doc.push(
                elements::Paragraph::new(format!("{}. {}", next_chapter + 1, chapter.title))
                    .styled(Style::new().bold().with_font_size(13)),
            );
            doc.push(elements::Break::new(0.5));
            next_chapter += 1;
        }

        let mut block = elements::Paragraph::default();
        block.push_styled(
            format!("[{}]  ", transcript::format_clock(paragraph.start_ms)),
            Style::new().bold(),
        );
        block.push(paragraph.text.clone());

        doc.push(block);
        doc.push(elements::Break::new(0.4));

        for screen in screens.iter().filter(|s| s.at_ms >= paragraph.start_ms && s.at_ms < ends_at) {
            let mut line = elements::Paragraph::default();
            line.push_styled(
                format!("Screen {}  ", transcript::format_clock(screen.at_ms)),
                Style::new().bold().with_font_size(9).with_color(MUTED),
            );
            line.push_styled(screen.lines.join(" · "), Style::new().with_font_size(9).with_color(MUTED));

            doc.push(line);
            doc.push(elements::Break::new(0.4));
        }

        doc.push(elements::Break::new(0.2));
    }
}

/// Page number, from page two onwards.
fn page_header(page: usize) -> impl Element {
    let mut layout = elements::LinearLayout::vertical();
    if page > 1 {
        layout.push(
            elements::Paragraph::new(page.to_string())
                .aligned(Alignment::Right)
                .styled(Style::new().with_font_size(8).with_color(MUTED)),
        );
    }
    layout.push(elements::Break::new(0.8));
    layout
}

/// The same facts the markdown header carries, one per line.
fn meta_lines(header: &TranscriptHeader<'_>) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(uploader) = header.uploader {
        lines.push(format!("Channel: {uploader}"));
    }
    lines.push(format!("Source: {}", header.url));
    if let Some(duration) = header.duration_seconds {
        lines.push(format!(
            "Duration: {}",
            transcript::format_clock((duration * 1000.0) as u64)
        ));
    }
    lines.push(format!(
        "Transcript from: {}{}",
        header.source,
        header.language.map(|l| format!(" ({l})")).unwrap_or_default()
    ));
    lines
}

fn load_font_family() -> Result<fonts::FontFamily<fonts::FontData>, String> {
    for faces in FONT_FAMILIES {
        if let Ok(family) = load_family(faces) {
            return Ok(family);
        }
    }
    Err(format!(
        "Could not load a font for the PDF. Expected Arial or Times New Roman in {FONT_DIR}."
    ))
}

fn load_family(faces: &[&str; 4]) -> Result<fonts::FontFamily<fonts::FontData>, String> {
    let mut loaded = Vec::with_capacity(faces.len());
    for face in faces {
        let bytes = std::fs::read(Path::new(FONT_DIR).join(face))
            .map_err(|e| format!("Could not read {face}: {e}"))?;
        loaded.push(
            fonts::FontData::new(bytes, None).map_err(|e| format!("{face} is unusable: {e}"))?,
        );
    }

    let mut faces = loaded.into_iter();
    Ok(fonts::FontFamily {
        regular: faces.next().expect("four faces loaded"),
        bold: faces.next().expect("four faces loaded"),
        italic: faces.next().expect("four faces loaded"),
        bold_italic: faces.next().expect("four faces loaded"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_readable_pdf_with_our_diacritics() {
        let path = std::env::temp_dir().join("yt-mp3-test.pdf");
        let _ = std::fs::remove_file(&path);
        let cues = vec![
            Cue { start_ms: 0, text: "Čeljust šuška, đak ćuti.".into() },
            Cue { start_ms: 61_000, text: "Druga minuta.".into() },
        ];
        let header = TranscriptHeader {
            title: "Proba",
            url: "/tmp/proba.mp3",
            uploader: None,
            duration_seconds: Some(122.0),
            source: "whisper",
            language: Some("auto"),
                    read_screen: false,
        };

        save_pdf(&path, &header, &cues, &[], &[]).unwrap();

        let written = std::fs::read(&path).unwrap();
        assert!(written.starts_with(b"%PDF"), "not a PDF");
        assert!(written.len() > 2000, "suspiciously small: {} bytes", written.len());

        let _ = std::fs::remove_file(&path);
    }
}
