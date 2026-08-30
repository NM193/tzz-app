//! Stitching a queue's transcripts into one document.
//!
//! Each video keeps its own file; this is the extra copy that reads as one
//! thing -- a course rather than ten downloads. Sections are built from the
//! per-video markdown, so the wording and the timestamps are identical to the
//! individual files. Nothing is re-transcribed and nothing can drift.

/// One video's contribution to the combined document.
pub struct Section {
    pub title: String,
    pub source: String,
    /// The per-video transcript document, header and all.
    pub markdown: String,
}

/// Build the combined markdown.
///
/// Timestamps restart at 00:00 in every section because they are positions
/// within that video, not within the course.
pub fn combined_markdown(name: &str, sections: &[Section]) -> String {
    let mut out = format!("# {name}\n\n- Combined from {} videos\n\n---\n", sections.len());

    for (index, section) in sections.iter().enumerate() {
        out.push_str(&format!("\n## {}. {}\n\n", index + 1, section.title));
        out.push_str(&format!("- Source: {}\n\n", section.source));

        for line in body_of(&section.markdown) {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// The paragraphs of a per-video document, without its own title and header.
///
/// Everything before the `---` rule is that file's own front matter, which
/// would only repeat what the section heading already says.
fn body_of(markdown: &str) -> Vec<&str> {
    let body = match markdown.split_once("\n---\n") {
        Some((_, rest)) => rest,
        None => markdown,
    };
    body.lines().filter(|line| !line.trim().is_empty()).collect()
}

/// Name the combined file after what the videos have in common.
///
/// Ten Stanford lectures share "Stanford CS229 Machine Learning", which makes
/// a far better filename than the first lecture's full title. When they share
/// nothing, fall back to something plain.
pub fn combined_name(titles: &[String]) -> String {
    let shared = shared_prefix_words(titles);
    let base = if shared.is_empty() { "Combined transcript".to_string() } else { shared };
    format!("{} ({} videos)", sanitise(&base), titles.len())
}

fn shared_prefix_words(titles: &[String]) -> String {
    let Some(first) = titles.first() else {
        return String::new();
    };

    let first_words: Vec<&str> = first.split_whitespace().collect();
    let mut shared = first_words.len();

    for title in titles.iter().skip(1) {
        let words: Vec<&str> = title.split_whitespace().collect();
        let mut common = 0;
        while common < shared && common < words.len() && words[common] == first_words[common] {
            common += 1;
        }
        shared = common;
    }

    // A single shared word is usually a coincidence, not a course name.
    if shared < 2 {
        return String::new();
    }

    cut_at_last_separator(&first_words[..shared].join(" "))
}

/// "Stanford CS229 Machine Learning | Lecture" -> "Stanford CS229 Machine Learning"
///
/// Titles in a series read "channel | series | episode", so the shared part
/// usually runs one separator too far and ends on a dangling word.
fn cut_at_last_separator(prefix: &str) -> String {
    let cut = match prefix.rfind(['|', '\u{2013}', '\u{2014}']) {
        Some(at) => prefix[..at].trim(),
        None => prefix.trim(),
    };

    if cut.split_whitespace().count() < 2 {
        return prefix.trim().to_string();
    }
    cut.to_string()
}

/// Keep the name usable as a filename on every platform.
pub fn sanitise(name: &str) -> String {
    name.chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(title: &str, body: &str) -> Section {
        Section {
            title: title.to_string(),
            source: format!("https://example.com/{title}"),
            markdown: format!("# {title}\n\n- Source: x\n\n---\n\n{body}\n"),
        }
    }

    #[test]
    fn numbers_the_sections_and_drops_their_own_front_matter() {
        let sections = vec![
            section("Lecture 1", "**[00:00]** first"),
            section("Lecture 2", "**[00:00]** second"),
        ];

        let out = combined_markdown("Course (2 videos)", &sections);

        assert!(out.starts_with("# Course (2 videos)"), "{out}");
        assert!(out.contains("## 1. Lecture 1"), "{out}");
        assert!(out.contains("## 2. Lecture 2"), "{out}");
        assert!(out.contains("**[00:00]** first"), "{out}");
        // The per-video "# Lecture 1" heading must not survive as a second title.
        assert!(!out.contains("\n# Lecture 1"), "{out}");
    }

    #[test]
    fn names_the_file_after_the_shared_course_title() {
        let titles = vec![
            "Stanford CS229 Machine Learning | Lecture 1: Introduction".to_string(),
            "Stanford CS229 Machine Learning | Lecture 2: Setup".to_string(),
        ];
        assert_eq!(combined_name(&titles), "Stanford CS229 Machine Learning (2 videos)");
    }

    #[test]
    fn falls_back_when_the_videos_share_nothing() {
        let titles = vec!["Cooking pasta".to_string(), "Rust lifetimes".to_string()];
        assert_eq!(combined_name(&titles), "Combined transcript (2 videos)");
    }

    #[test]
    fn keeps_the_name_usable_as_a_filename() {
        let titles = vec!["A/B testing explained".to_string(), "A/B testing pitfalls".to_string()];
        assert_eq!(combined_name(&titles), "A-B testing (2 videos)");
    }
}
