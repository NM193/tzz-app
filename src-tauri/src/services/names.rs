//! What a job's folder in the library is called, and whether it is taken.
//!
//! The job and the check before Start both name folders here, so the two
//! cannot disagree. Taken means the folder already holds a result -- an MP3
//! or a transcript -- not merely that it exists: a failed run leaves a folder
//! of leftovers, and running it again must not be asked about.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

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

/// One job about to join the queue, as the check before Start sees it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The frontend's id for the job, handed back unchanged.
    pub key: String,
    /// A file on disk. Its stem names the folder unless `name` says otherwise.
    pub path: Option<String>,
    /// A link's title.
    pub title: Option<String>,
    /// A link's video id. A link without one is not checked.
    pub video_id: Option<String>,
    /// The name in the dialog's field, which wins over the others.
    pub name: Option<String>,
}

/// What the check found for one job. Mirrors `Clash` in `src/lib/api.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clash {
    pub key: String,
    /// The folder the job would write into; empty when it was not checked.
    pub folder: String,
    /// The library already has that folder, holding a result.
    pub in_library: bool,
    /// The key of an earlier job in this batch that writes into it too.
    pub same_as: Option<String>,
    /// Replace is on offer: the library has the folder, no earlier job
    /// writes into it, and no other job reads its file from there.
    pub replaceable: bool,
    /// The first free `<name> (n)`, when the folder is taken.
    pub suggestion: Option<String>,
    /// The name can be a folder at all.
    pub usable: bool,
    pub holds: Holds,
}

// The tests' shorthand. The app asks the same question in `naming.ts`.
#[cfg(test)]
impl Clash {
    fn is_taken(&self) -> bool {
        self.in_library || self.same_as.is_some()
    }
}

/// The name a job's folder is made from, and the id it carries.
fn naming(item: &Candidate) -> Option<(String, Option<&str>)> {
    if let Some(path) = item.path.as_deref() {
        let name = item.name.clone().unwrap_or_else(|| file_title(Path::new(path)));
        return Some((name, None));
    }
    let id = item.video_id.as_deref()?;
    let name = item.name.clone().or_else(|| item.title.clone())?;
    Some((name, Some(id)))
}

/// Every job of a batch against the library and against each other, in
/// order, one answer each.
///
/// Names compare case-folded, as APFS compares them. Suggestions stay clear
/// of every folder the batch uses, above or below, and of each other.
pub fn clashes(root: &Path, items: &[Candidate]) -> Vec<Clash> {
    let named: Vec<Option<(String, Option<&str>)>> = items.iter().map(naming).collect();
    let folders: Vec<Option<String>> = named
        .iter()
        .map(|n| n.as_ref().map(|(name, id)| folder_name(name, *id)))
        .collect();
    let mut used: HashSet<String> = folders.iter().flatten().map(|f| f.to_lowercase()).collect();

    items
        .iter()
        .enumerate()
        .map(|(at, item)| {
            let (Some((name, id)), Some(folder)) = (&named[at], &folders[at]) else {
                return Clash {
                    key: item.key.clone(),
                    folder: String::new(),
                    in_library: false,
                    same_as: None,
                    replaceable: false,
                    suggestion: None,
                    usable: true,
                    holds: Holds::default(),
                };
            };
            let dir = root.join(folder);
            let reads_from = |candidate: &Candidate| {
                candidate.path.as_deref().is_some_and(|p| source_inside(Path::new(p), &dir))
            };

            let lower = folder.to_lowercase();
            let same_as = (0..at)
                .find(|&k| folders[k].as_ref().is_some_and(|f| f.to_lowercase() == lower))
                .map(|k| items[k].key.clone());
            let found = holds(&dir);
            let in_library = !reads_from(item) && found.has_result();
            let read_by_another =
                items.iter().enumerate().any(|(k, other)| k != at && reads_from(other));

            let suggestion = (in_library || same_as.is_some()).then(|| {
                let free = (2..)
                    .map(|n| suggest(name, *id, n))
                    .find(|s| {
                        let f = folder_name(s, *id);
                        !used.contains(&f.to_lowercase()) && !root.join(&f).exists()
                    })
                    .expect("one of endlessly many numbers is free");
                used.insert(folder_name(&free, *id).to_lowercase());
                free
            });

            Clash {
                key: item.key.clone(),
                folder: folder.clone(),
                in_library,
                replaceable: in_library && same_as.is_none() && !read_by_another,
                same_as,
                suggestion,
                usable: usable(name),
                holds: found,
            }
        })
        .collect()
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

    fn link(key: &str, title: &str, id: &str) -> Candidate {
        Candidate {
            key: key.into(),
            path: None,
            title: Some(title.into()),
            video_id: Some(id.into()),
            name: None,
        }
    }

    fn file(key: &str, path: &Path) -> Candidate {
        Candidate {
            key: key.into(),
            path: Some(path.to_string_lossy().into_owned()),
            title: None,
            video_id: None,
            name: None,
        }
    }

    #[test]
    fn a_finished_link_is_in_the_library() {
        let root = temp_root("link");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3", "Talk [abc].notes.md"]);
        let found = clashes(&root, &[link("1", "Talk", "abc"), link("2", "Other", "def")]);
        assert!(found[0].in_library && found[0].replaceable);
        assert_eq!(found[0].folder, "Talk [abc]");
        assert_eq!(found[0].suggestion.as_deref(), Some("Talk (2)"));
        assert!(found[0].holds.audio && found[0].holds.notes);
        assert!(!found[1].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_run_is_not_asked_about() {
        let root = temp_root("failed");
        folder_with(&root, "Talk [abc]", &["Talk [abc].webm", "poster.jpg"]);
        assert!(!clashes(&root, &[link("1", "Talk", "abc")])[0].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_link_without_an_id_is_not_checked() {
        let root = temp_root("noid");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3"]);
        let mut item = link("1", "Talk", "abc");
        item.video_id = None;
        let found = clashes(&root, &[item]);
        assert!(!found[0].is_taken());
        assert!(found[0].folder.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_files_of_one_name_clash_whatever_the_case() {
        let root = temp_root("batch");
        let found = clashes(
            &root,
            &[file("a", Path::new("/w1/Lecture 1.mp4")), file("b", Path::new("/w2/lecture 1.mp4"))],
        );
        // The library has neither: the first is free, the second is not.
        assert!(!found[0].is_taken());
        assert_eq!(found[1].same_as.as_deref(), Some("a"));
        assert!(!found[1].replaceable);
        assert_eq!(found[1].suggestion.as_deref(), Some("lecture 1 (2)"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn suggestions_skip_every_name_the_batch_uses() {
        let root = temp_root("suggest");
        folder_with(&root, "Lecture 1", &["Lecture 1.mp3"]);
        let found = clashes(
            &root,
            &[
                file("a", Path::new("/w1/Lecture 1.mp4")),
                file("b", Path::new("/w2/Lecture 1.mp4")),
                file("c", Path::new("/w3/Lecture 1 (2).mp4")),
            ],
        );
        // (2) belongs to c, further down; a takes (3) and b the next free.
        assert_eq!(found[0].suggestion.as_deref(), Some("Lecture 1 (3)"));
        assert_eq!(found[1].suggestion.as_deref(), Some("Lecture 1 (4)"));
        assert!(!found[2].is_taken());
        // Only the first of a folder's jobs may replace it.
        assert!(found[0].replaceable && !found[1].replaceable);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_dropped_from_its_own_folder_is_not_taken() {
        let root = temp_root("own");
        let folder = folder_with(&root, "Lecture 1", &["Lecture 1.mp3", "Lecture 1.md"]);
        assert!(!clashes(&root, &[file("a", &folder.join("Lecture 1.mp3"))])[0].is_taken());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn replace_is_not_offered_when_another_job_reads_from_the_folder() {
        let root = temp_root("reads");
        let folder = folder_with(&root, "Talk [abc]", &["Talk [abc].mp3", "Talk [abc].mp4"]);
        let found = clashes(
            &root,
            &[link("link", "Talk", "abc"), file("kept", &folder.join("Talk [abc].mp4"))],
        );
        assert!(found[0].in_library);
        assert!(!found[0].replaceable, "Replace would delete the video the next job reads");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_from_the_dialog_wins_and_is_judged() {
        let root = temp_root("chosen");
        folder_with(&root, "Talk [abc]", &["Talk [abc].mp3"]);
        let mut renamed = link("1", "Talk", "abc");
        renamed.name = Some("Talk (2)".into());
        let found = clashes(&root, &[renamed]);
        assert_eq!(found[0].folder, "Talk (2) [abc]");
        assert!(!found[0].is_taken());

        let mut hidden = link("2", "Talk", "abc");
        hidden.name = Some(".secret".into());
        assert!(!clashes(&root, &[hidden])[0].usable);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_clash_names_the_job_it_clashes_with_even_below() {
        let root = temp_root("below");
        let mut renamed = file("row", Path::new("/w1/Lecture 1.mp4"));
        renamed.name = Some("Lecture 2".into());
        let found = clashes(&root, &[renamed, file("other", Path::new("/w2/Lecture 2.mp4"))]);
        assert_eq!(found[1].same_as.as_deref(), Some("row"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
