//! What a job's folder in the library is called, and whether it is taken.
//!
//! The job and the check before Start both name folders here, so the two
//! cannot disagree. Taken means the folder already holds a result -- an MP3
//! or a transcript -- not merely that it exists: a failed run leaves a folder
//! of leftovers, and running it again must not be asked about.

use std::path::Path;

use serde::Serialize;

use super::combine;
use super::library;

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

/// What a folder holds, so the dialog can say what Replace would take.
#[derive(Debug, Default, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Holds {
    /// An MP3.
    pub audio: bool,
    /// A transcript: markdown other than the notes, or a PDF.
    pub transcript: bool,
    pub video: bool,
    /// The cache of what was read off the screen.
    pub screens: bool,
    pub notes: bool,
    /// Seconds since the epoch: when anything in it last changed.
    pub modified: u64,
}

impl Holds {
    /// Taken: the folder holds an MP3 or a transcript. Leftovers of a
    /// failed run, a kept video, or notes alone do not count.
    pub fn has_result(&self) -> bool {
        self.audio || self.transcript
    }
}

pub fn holds(folder: &Path) -> Holds {
    let mut found = Holds::default();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let ext = name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
        if name.ends_with(".notes.md") {
            found.notes = true;
        } else if name.ends_with(".screens.json") {
            found.screens = true;
        } else if ext == "mp3" {
            found.audio = true;
        } else if ext == "md" || ext == "pdf" {
            found.transcript = true;
        } else if library::VIDEO.contains(&ext) {
            found.video = true;
        }
        found.modified = found.modified.max(library::modified_at(&path));
    }
    found
}

/// Whether two paths name one folder. By inode, as `mp3.rs` compares files:
/// the volume ignores case, so "Lecture 1" and "lecture 1" are one folder.
fn same_dir(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// A file being made again from inside the folder it would go to: dropped
/// from its own library folder. Replace there would delete it unread.
pub fn source_inside(source: &Path, folder: &Path) -> bool {
    source.parent().is_some_and(|parent| same_dir(parent, folder))
}

/// Empty a folder for Replace. Every file goes but the notes, the one thing
/// in it the app cannot make again. Only a folder directly inside the
/// library is touched.
pub fn clear_for_replace(root: &Path, folder: &Path) -> Result<(), String> {
    if !folder.parent().is_some_and(|parent| same_dir(parent, root)) {
        return Err("That folder is not in the library.".to_string());
    }
    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("Could not clear the old results: {e}"))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let notes = entry.file_name().to_string_lossy().to_lowercase().ends_with(".notes.md");
        if path.is_file() && !notes {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Could not clear the old results: {e}"))?;
        }
    }
    Ok(())
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

    fn temp_root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("tzz-names-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn folder_with(root: &Path, name: &str, files: &[&str]) -> std::path::PathBuf {
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        for file in files {
            std::fs::write(folder.join(file), b"x").unwrap();
        }
        folder
    }

    #[test]
    fn a_result_is_an_mp3_or_a_transcript() {
        let root = temp_root("result");
        let result = |folder: std::path::PathBuf| holds(&folder).has_result();
        assert!(result(folder_with(&root, "a", &["a.mp3"])));
        assert!(result(folder_with(&root, "b", &["b.md"])));
        assert!(result(folder_with(&root, "c", &["c.pdf"])));
        // What a failed run leaves, a kept video, notes: none is a result.
        assert!(!result(folder_with(&root, "d", &["d.webm", "d.en-orig.vtt", "poster.jpg"])));
        assert!(!result(folder_with(&root, "e", &["e.notes.md"])));
        assert!(!result(root.join("missing")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn says_what_a_folder_holds() {
        let root = temp_root("holds");
        let folder = folder_with(
            &root,
            "L [id]",
            &["L [id].mp3", "L [id].md", "L [id].notes.md", "L [id].screens.json", "L [id].mp4"],
        );
        let found = holds(&folder);
        assert!(found.audio && found.transcript && found.notes && found.screens && found.video);
        assert!(found.modified > 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_inside_its_own_folder_is_seen_whatever_the_case() {
        let root = temp_root("inside");
        let folder = folder_with(&root, "Lecture 1", &["Lecture 1.mp3"]);
        assert!(source_inside(&folder.join("Lecture 1.mp3"), &folder));
        // APFS ignores case: the same folder spelled another way.
        assert!(source_inside(&folder.join("Lecture 1.mp3"), &root.join("lecture 1")));
        assert!(!source_inside(Path::new("/tmp/elsewhere/Lecture 1.mp3"), &folder));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_keeps_the_notes_and_nothing_else() {
        let root = temp_root("clear");
        let folder =
            folder_with(&root, "L", &["L.mp3", "L.md", "L.notes.md", "L.screens.json", "poster.jpg"]);
        clear_for_replace(&root, &folder).unwrap();
        let left: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["L.notes.md".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_touches_nothing_outside_the_library() {
        let root = temp_root("outside");
        let elsewhere = temp_root("outside-elsewhere");
        let folder = folder_with(&elsewhere, "L", &["L.mp3"]);
        assert!(clear_for_replace(&root, &folder).is_err());
        assert!(folder.join("L.mp3").exists());
        // Nor anything deeper than one level, nor the library itself.
        let nested = folder_with(&root, "a/b", &["b.mp3"]);
        assert!(clear_for_replace(&root, &nested).is_err());
        assert!(clear_for_replace(&root, &root).is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }
}
