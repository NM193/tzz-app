//! Reading a transcript back out of its own markdown.
//!
//! The app writes these files (`transcript.rs`), so parsing them is not a
//! markdown problem -- it is reading back a shape we chose. Only what the
//! writer produces is understood, and anything unrecognised is kept as plain
//! text rather than dropped: a document the user edited by hand must still
//! open.
//!
//! This lives in Rust rather than the frontend so that the writer and the
//! reader sit next to each other, and so the shape is covered by tests.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Block {
    /// What was said, with the moment it was said.
    Said { at: String, text: String },
    /// What was on screen at that moment, read by OCR.
    Screen { at: String, text: String },
    /// A line the writer did not produce; shown as it is.
    Plain { text: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    /// "3. Figma File" keeps its number; a document without chapters has one
    /// unnamed chapter holding everything.
    pub title: String,
    pub at: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub title: String,
    /// The "- Channel: ..." lines under the title, without their dashes.
    pub meta: Vec<String>,
    pub chapters: Vec<Chapter>,
    /// Every word that was said, for counting and for searching.
    pub words: usize,
}

pub fn parse(markdown: &str) -> Document {
    let mut title = String::new();
    let mut meta: Vec<String> = Vec::new();
    let mut chapters: Vec<Chapter> = Vec::new();
    let mut words = 0usize;
    // The table of contents repeats the chapter list; the chapters themselves
    // are the real thing, so it is skipped.
    let mut in_contents = false;

    for raw in markdown.lines() {
        let line = raw.trim();
        if line.is_empty() || line == "---" {
            continue;
        }

        if let Some(rest) = line.strip_prefix("# ") {
            title = rest.trim().to_string();
            continue;
        }

        if let Some(rest) = line.strip_prefix("## ") {
            let heading = rest.trim();
            in_contents = heading.eq_ignore_ascii_case("contents");
            if !in_contents {
                chapters.push(Chapter { title: heading.to_string(), at: None, blocks: Vec::new() });
            }
            continue;
        }

        if in_contents {
            continue;
        }

        // Front matter, before any chapter has started.
        if chapters.is_empty() {
            if let Some(rest) = line.strip_prefix("- ") {
                meta.push(rest.trim().to_string());
                continue;
            }
        }

        let chapter = match chapters.last_mut() {
            Some(chapter) => chapter,
            None => {
                chapters.push(Chapter { title: String::new(), at: None, blocks: Vec::new() });
                chapters.last_mut().expect("just pushed")
            }
        };

        // A bare "*00:35*" is the minute marker the writer puts under a
        // heading. It is the chapter's time, not a paragraph.
        if let Some(clock) = line.strip_prefix('*').and_then(|l| l.strip_suffix('*')) {
            if is_clock(clock) {
                if chapter.at.is_none() {
                    chapter.at = Some(clock.to_string());
                }
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("**[") {
            if let Some((clock, text)) = rest.split_once("]**") {
                let text = text.trim();
                words += text.split_whitespace().count();
                chapter
                    .blocks
                    .push(Block::Said { at: clock.to_string(), text: text.to_string() });
                continue;
            }
        }

        if let Some(rest) = line.strip_prefix("> **Screen ") {
            if let Some((clock, text)) = rest.split_once("** -- ") {
                chapter
                    .blocks
                    .push(Block::Screen { at: clock.trim().to_string(), text: text.trim().to_string() });
                continue;
            }
        }

        chapter.blocks.push(Block::Plain { text: line.to_string() });
    }

    Document { title, meta, chapters, words }
}

fn is_clock(text: &str) -> bool {
    !text.is_empty()
        && text.split(':').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        && text.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "\
# Lumos Crash Course

- Channel: Timothy Ricks
- Duration: 1:04:30

---

## Contents

1. [Intro](#1-intro) — 00:00
2. [Figma File](#2-figma-file) — 01:14

---

## 1. Intro

*00:00*

**[00:00]** In this lesson you will learn the framework.

> **Screen 00:34** -- Lumos · Consistency at Scale

## 2. Figma File

*01:14*

**[01:14]** Open the variables panel.
";

    #[test]
    fn reads_the_title_and_the_front_matter() {
        let doc = parse(DOC);
        assert_eq!(doc.title, "Lumos Crash Course");
        assert_eq!(doc.meta, vec!["Channel: Timothy Ricks", "Duration: 1:04:30"]);
    }

    #[test]
    fn the_table_of_contents_is_not_a_chapter() {
        let doc = parse(DOC);
        let titles: Vec<&str> = doc.chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["1. Intro", "2. Figma File"]);
    }

    #[test]
    fn a_chapter_takes_its_time_from_the_minute_marker() {
        let doc = parse(DOC);
        assert_eq!(doc.chapters[0].at.as_deref(), Some("00:00"));
        assert_eq!(doc.chapters[1].at.as_deref(), Some("01:14"));
    }

    #[test]
    fn what_was_said_and_what_was_on_screen_stay_apart() {
        let doc = parse(DOC);
        assert_eq!(
            doc.chapters[0].blocks,
            vec![
                Block::Said {
                    at: "00:00".into(),
                    text: "In this lesson you will learn the framework.".into()
                },
                Block::Screen {
                    at: "00:34".into(),
                    text: "Lumos · Consistency at Scale".into()
                },
            ]
        );
    }

    #[test]
    fn counts_only_the_words_that_were_said() {
        // The screen line and the headings do not count.
        assert_eq!(parse(DOC).words, 8 + 4);
    }

    #[test]
    fn a_document_with_no_chapters_still_opens() {
        let doc = parse("# Notes\n\n**[00:00]** hello there\n");
        assert_eq!(doc.chapters.len(), 1);
        assert_eq!(doc.chapters[0].title, "");
        assert_eq!(doc.words, 2);
    }

    #[test]
    fn a_line_we_did_not_write_is_kept_as_it_is() {
        let doc = parse("# Notes\n\n## One\n\nsomething typed by hand\n");
        assert_eq!(
            doc.chapters[0].blocks,
            vec![Block::Plain { text: "something typed by hand".into() }]
        );
    }
}
