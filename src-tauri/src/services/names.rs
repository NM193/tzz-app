//! What a job's folder in the library is called, and whether it is taken.
//!
//! The job and the check before Start both name folders here, so the two
//! cannot disagree. Taken means the folder already holds a result -- an MP3
//! or a transcript -- not merely that it exists: a failed run leaves a folder
//! of leftovers, and running it again must not be asked about.

use std::path::Path;

use super::combine;

/// The longest a folder's name gets, `[id]` and `(n)` included. It was the
/// limit before there was a check, and it keeps a name, plus what the files
/// inside add to it, clear of the 255-byte filename limit.
const LIMIT: usize = 120;

/// Sanitised, cut to `room` characters, trimmed.
fn cut(name: &str, room: usize) -> String {
    combine::sanitise(name).chars().take(room).collect::<String>().trim().to_string()
}

fn id_part(id: Option<&str>) -> String {
    id.map(|id| format!(" [{id}]")).unwrap_or_default()
}

/// The folder a job writes into: the name cut to fit, then ` [id]` for a
/// link. Cut first, so a long name keeps its `[id]` -- `thumb.rs` finds the
/// cover through it.
pub fn folder_name(name: &str, id: Option<&str>) -> String {
    let id = id_part(id);
    format!("{}{id}", cut(name, LIMIT.saturating_sub(id.chars().count())))
}

/// `<name> (n)`, cut so the whole of it fits as `folder_name` would cut it.
/// It comes back through `folder_name` unchanged, which is what lets it
/// travel to the job as a plain name and keep its number.
pub fn suggest(name: &str, id: Option<&str>, n: usize) -> String {
    let number = format!(" ({n})");
    let room = LIMIT.saturating_sub(id_part(id).chars().count() + number.chars().count());
    format!("{}{number}", cut(name, room)).trim().to_string()
}

/// A name that can be a folder in the library: something is left after
/// sanitising, and it is not hidden. `..` would leave the library, and a
/// leading dot would hide the folder from the Library screen.
pub fn usable(name: &str) -> bool {
    let base = cut(name, LIMIT);
    !base.is_empty() && !base.starts_with('.')
}

/// A file's name without its extension, as its library folder is named.
pub fn file_title(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Audio".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_what_they_were_before() {
        // What job_folder and library_folder made before this module existed:
        // the whole name, `[id]` included, cut to 120 characters and trimmed.
        let old = |name: &str| name.chars().take(120).collect::<String>().trim().to_string();
        let title = "Lumos V2.1 Crash Course (Webflow Framework)";
        assert_eq!(folder_name(title, Some("GPR123")), old(&format!("{title} [GPR123]")));
        assert_eq!(folder_name("A/B testing: what works", Some("x1")), "A-B testing- what works [x1]");
        assert_eq!(folder_name("Lecture 1", None), "Lecture 1");
        let long = "x".repeat(200);
        assert_eq!(folder_name(&long, None), old(&long));
    }

    #[test]
    fn a_long_name_keeps_its_id() {
        let name = folder_name(&"y".repeat(200), Some("abc123"));
        assert!(name.ends_with(" [abc123]"), "{name}");
        assert_eq!(name.chars().count(), 120);
    }

    #[test]
    fn a_long_name_keeps_its_number() {
        let long = "z".repeat(200);
        let (two, three) = (suggest(&long, None, 2), suggest(&long, None, 3));
        assert!(two.ends_with(" (2)") && three.ends_with(" (3)"));
        assert_ne!(folder_name(&two, None), folder_name(&three, None));
    }

    #[test]
    fn a_suggestion_comes_back_unchanged() {
        let (w, v) = ("w".repeat(300), "v".repeat(300));
        let cases: [(&str, Option<&str>); 4] =
            [("Lecture 1", None), (&w, None), (&v, Some("id9")), ("Talk", Some("id9"))];
        for (name, id) in cases {
            for n in [2, 9, 10, 123] {
                let suggestion = suggest(name, id, n);
                let expected = format!("{suggestion}{}", id_part(id));
                assert_eq!(folder_name(&suggestion, id), expected, "{suggestion}");
            }
        }
    }

    #[test]
    fn a_suggestion_is_numbered_and_trimmed() {
        assert_eq!(suggest("Lecture 1", None, 2), "Lecture 1 (2)");
        // A cut that ends on a space does not leave two before the number.
        let spaced = format!("{} tail", "a".repeat(115));
        assert!(!suggest(&spaced, None, 2).contains("  ("));
    }

    #[test]
    fn hidden_or_empty_names_cannot_be_folders() {
        for bad in ["", "   ", ".", "..", ".hidden"] {
            assert!(!usable(bad), "{bad:?}");
        }
        for good in ["Lecture 1", "A/B", "1. Intro"] {
            assert!(usable(good), "{good:?}");
        }
    }

    #[test]
    fn a_file_is_named_by_its_stem() {
        assert_eq!(file_title(Path::new("/x/1. Intro.mp4")), "1. Intro");
    }
}
