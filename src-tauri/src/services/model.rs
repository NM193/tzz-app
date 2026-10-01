//! Fetching a Whisper model, because one cannot travel inside the app.
//!
//! The smallest usable model is 141 MB and the one worth having is 1.5 GB, so
//! a download on a fresh Mac is the only sensible way. There is no HTTP client
//! in this project and no reason to add one: `/usr/bin/curl` is on every Mac
//! and is on the PATH even for an app launched from Finder.
//!
//! Progress is the size of the part file against the size we expect, polled,
//! rather than anything parsed out of curl's own output. It needs no format to
//! stay stable and is right even when the connection stalls.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use super::cancel;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    /// What it costs you, in one line.
    pub note: &'static str,
    pub bytes: u64,
    pub installed: bool,
}

struct Known {
    id: &'static str,
    file: &'static str,
    name: &'static str,
    note: &'static str,
    bytes: u64,
}

/// Measured against Hugging Face on 2026-10-01. The size is only used to draw
/// the progress bar, so being a little off costs nothing.
const KNOWN: &[Known] = &[
    Known {
        id: "base",
        file: "ggml-base.bin",
        name: "Base",
        note: "Quick, and good enough to find your way around a lecture.",
        bytes: 148 * 1_000_000,
    },
    Known {
        id: "small",
        file: "ggml-small.bin",
        name: "Small",
        note: "A fair middle. Noticeably better on accents and names.",
        bytes: 488 * 1_000_000,
    },
    Known {
        id: "large-v3-turbo",
        file: "ggml-large-v3-turbo.bin",
        name: "Large v3 Turbo",
        note: "The one to keep. About as fast as Small on an M-series Mac.",
        bytes: 1624 * 1_000_000,
    },
];

const HOST: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Where models live. The app has looked here since the first version.
pub fn folder() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/yt-mp3/models"),
    )
}

fn path_for(known: &Known) -> Option<PathBuf> {
    Some(folder()?.join(known.file))
}

/// Every model offered, and whether it is already here.
pub fn catalogue() -> Vec<Model> {
    KNOWN
        .iter()
        .map(|known| Model {
            id: known.id,
            name: known.name,
            note: known.note,
            bytes: known.bytes,
            installed: path_for(known).is_some_and(|path| path.is_file()),
        })
        .collect()
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: String,
    percent: f32,
}

/// Fetch a model, reporting how far along it is.
///
/// It lands under a `.part` name and is only moved into place once curl says
/// it finished: a half-written model that looks installed is worse than none,
/// because every later job would fail on it instead.
pub async fn download(app: &AppHandle, id: &str) -> Result<String, String> {
    let known = KNOWN
        .iter()
        .find(|known| known.id == id)
        .ok_or_else(|| "No such model.".to_string())?;

    let folder = folder().ok_or_else(|| "Could not find your home folder.".to_string())?;
    std::fs::create_dir_all(&folder)
        .map_err(|e| format!("Could not make the models folder: {e}"))?;

    let destination = folder.join(known.file);
    if destination.is_file() {
        return Ok(destination.to_string_lossy().into_owned());
    }

    let part = folder.join(format!("{}.part", known.file));
    let _ = std::fs::remove_file(&part);
    cancel::claim(&part);

    let mut child = Command::new("/usr/bin/curl")
        .args(["-fL", "--retry", "3", "-o"])
        .arg(&part)
        .arg(format!("{HOST}/{}", known.file))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Could not start the download: {e}"))?;

    let tracked = cancel::track(child.id());
    let watching = watch(app.clone(), id.to_string(), part.clone(), known.bytes);

    let status = child.wait().await;
    watching.abort();
    drop(tracked);

    let ok = status.map(|status| status.success()).unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_file(&part);
        cancel::check()?;
        return Err("The download did not finish. Check the connection and try again.".to_string());
    }

    std::fs::rename(&part, &destination)
        .map_err(|e| format!("Could not put the model in place: {e}"))?;

    let _ = app.emit("model://progress", Progress { id: id.to_string(), percent: 100.0 });
    Ok(destination.to_string_lossy().into_owned())
}

/// Watch the part file grow. Dropped as soon as the download ends.
fn watch(app: AppHandle, id: String, part: PathBuf, total: u64) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let percent = percent_of(&part, total);
            // A dropped tick is nothing; never let it end the download.
            let _ = app.emit("model://progress", Progress { id: id.clone(), percent });
        }
    })
}

fn percent_of(part: &Path, total: u64) -> f32 {
    let so_far = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if total == 0 {
        return 0.0;
    }
    ((so_far as f64 / total as f64) * 100.0).min(99.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_has_a_file_and_a_size() {
        for known in KNOWN {
            assert!(known.file.starts_with("ggml-"), "{}", known.file);
            assert!(known.bytes > 100_000_000, "{} looks too small", known.id);
        }
    }

    #[test]
    fn the_catalogue_says_what_is_already_here() {
        let listed = catalogue();
        assert_eq!(listed.len(), KNOWN.len());
        assert!(listed.iter().any(|model| model.id == "large-v3-turbo"));
    }

    #[test]
    fn progress_never_claims_to_be_finished() {
        let missing = Path::new("/definitely/not/here.part");
        assert_eq!(percent_of(missing, 1000), 0.0);
        // Only the rename at the end may say 100.
        assert!(percent_of(Path::new("/etc/hosts"), 1) <= 99.0);
    }
}
