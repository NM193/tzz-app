//! Your own notes, kept beside the transcript.
//!
//! The app does not write these. You copy a chapter out, ask whatever you like
//! to make something of it, and paste the answer back against that chapter.
//! What the app does is keep them: one file per lecture, each note under the
//! heading of the chapter it belongs to, so they survive, can be read here and
//! can be handed on as a document like any other.
//!
//! Chapters are addressed by their title rather than their position. A
//! transcript can be made again, and the titles come back the same while
//! nothing guarantees the file does.

/// One chapter's note.
pub struct Note<'a> {
    pub chapter: &'a str,
    pub body: &'a str,
}

/// Read a notes file into its sections, in the order they appear.
pub fn parse(markdown: &str) -> Vec<Note<'_>> {
    let mut notes = Vec::new();
    let mut chapter: Option<&str> = None;
    let mut start = 0usize;
    let mut cursor = 0usize;

    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(title) = chapter {
                notes.push(Note { chapter: title, body: markdown[start..cursor].trim() });
            }
            chapter = Some(rest.trim());
            start = cursor + line.len();
        }
        cursor += line.len();
    }

    if let Some(title) = chapter {
        notes.push(Note { chapter: title, body: markdown[start..].trim() });
    }

    notes.into_iter().filter(|note| !note.body.is_empty()).collect()
}

/// Put a note against a chapter, replacing whatever was there.
///
/// An empty body removes the section: clearing a note is how you delete one,
/// and an empty heading left behind would read as a note that says nothing.
pub fn upsert(markdown: &str, chapter: &str, body: &str) -> String {
    let body = body.trim();
    let mut out = String::new();
    let mut replaced = false;

    for note in parse(markdown) {
        if note.chapter == chapter {
            replaced = true;
            if !body.is_empty() {
                out.push_str(&section(chapter, body));
            }
            continue;
        }
        out.push_str(&section(note.chapter, note.body));
    }

    if !replaced && !body.is_empty() {
        out.push_str(&section(chapter, body));
    }

    out
}

fn section(chapter: &str, body: &str) -> String {
    format!("## {chapter}\n\n{body}\n\n")
}

/// The file as it is written: a title, then every note.
pub fn document(title: &str, markdown: &str) -> String {
    format!("# {title}\n\n- Notes written by hand\n\n---\n\n{}", markdown.trim_start())
}

/// The body of a notes file, without the front matter `document` adds.
pub fn body_of(markdown: &str) -> &str {
    match markdown.split_once("\n---\n") {
        Some((_, rest)) => rest,
        None => markdown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: &str = "## 1. Intro\n\nIt teaches grids.\n\n## 2. Figma File\n\nOpen the panel.\n\n";

    #[test]
    fn reads_every_note_under_its_own_chapter() {
        let notes = parse(NOTES);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].chapter, "1. Intro");
        assert_eq!(notes[0].body, "It teaches grids.");
        assert_eq!(notes[1].chapter, "2. Figma File");
    }

    #[test]
    fn a_new_note_goes_on_the_end() {
        let out = upsert(NOTES, "3. Grids", "Auto fit, with a max column count.");
        let notes = parse(&out);
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[2].chapter, "3. Grids");
    }

    #[test]
    fn writing_over_a_note_keeps_its_place() {
        let out = upsert(NOTES, "1. Intro", "Something better.");
        let notes = parse(&out);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].chapter, "1. Intro");
        assert_eq!(notes[0].body, "Something better.");
        assert_eq!(notes[1].chapter, "2. Figma File");
    }

    #[test]
    fn clearing_a_note_removes_it() {
        let out = upsert(NOTES, "1. Intro", "   ");
        let notes = parse(&out);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].chapter, "2. Figma File");
    }

    #[test]
    fn a_note_may_run_to_several_paragraphs() {
        let out = upsert("", "1. Intro", "First line.\n\nSecond line.");
        assert_eq!(parse(&out)[0].body, "First line.\n\nSecond line.");
    }

    #[test]
    fn the_front_matter_is_not_a_note() {
        let written = document("Lecture", NOTES);
        assert_eq!(parse(body_of(&written)).len(), 2);
    }

    #[test]
    fn an_empty_file_has_no_notes() {
        assert!(parse("").is_empty());
        assert!(parse("# Title\n\nsome preamble\n").is_empty());
    }
}
