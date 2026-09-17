//! Stopping a job part-way, and taking its half-written files with it.
//!
//! One job runs at a time, so this is a single global rather than a handle
//! threaded through every download and transcription call. A stop does three
//! things: raises a flag the job checks between stages, kills the external
//! processes the job is waiting on, and remembers what to delete.
//!
//! WHY delete: a stopped job leaves a folder with a half-downloaded video or a
//! transcript with no ending. That looks finished in Finder and is not. The
//! rule is simple -- anything the job created goes -- and it is applied by
//! time: a folder that existed before the job keeps its older files.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

pub const STOPPED: &str = "Stopped.";

struct Control {
    cancelled: AtomicBool,
    /// Process groups to signal. Every child is its own group leader, so the
    /// helpers a tool spawns (yt-dlp's ffmpeg) go down with it.
    groups: Mutex<Vec<u32>>,
    /// Paths the running job may have created, with whether each existed
    /// before it started.
    claims: Mutex<Vec<(PathBuf, bool)>>,
}

static CONTROL: Control = Control {
    cancelled: AtomicBool::new(false),
    groups: Mutex::new(Vec::new()),
    claims: Mutex::new(Vec::new()),
};

/// Call at the start of a job. Returns the moment it began, for `finish`.
pub fn begin() -> SystemTime {
    CONTROL.cancelled.store(false, Ordering::SeqCst);
    CONTROL.groups.lock().map(|mut g| g.clear()).ok();
    CONTROL.claims.lock().map(|mut c| c.clear()).ok();
    SystemTime::now()
}

/// Stop the running job. Safe to call when nothing is running.
pub fn cancel() {
    CONTROL.cancelled.store(true, Ordering::SeqCst);
    let groups = CONTROL.groups.lock().map(|g| g.clone()).unwrap_or_default();
    for pgid in groups {
        // A negative pid addresses the whole group. TERM rather than KILL so
        // the tools close their files instead of leaving them locked.
        let _ = Command::new("/bin/kill")
            .args(["-TERM", &format!("-{pgid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

pub fn is_cancelled() -> bool {
    CONTROL.cancelled.load(Ordering::SeqCst)
}

/// The check a job makes between stages.
pub fn check() -> Result<(), String> {
    if is_cancelled() {
        Err(STOPPED.to_string())
    } else {
        Ok(())
    }
}

/// Watch a child process for the length of the returned guard.
pub fn track(pid: Option<u32>) -> Tracked {
    if let Some(pid) = pid {
        CONTROL.groups.lock().map(|mut g| g.push(pid)).ok();
    }
    Tracked(pid)
}

pub struct Tracked(Option<u32>);

impl Drop for Tracked {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            CONTROL.groups.lock().map(|mut g| g.retain(|&p| p != pid)).ok();
        }
    }
}

/// Note a path the job is about to write, so a stop can take it back.
pub fn claim(path: &Path) {
    let existed = path.exists();
    CONTROL.claims.lock().map(|mut c| c.push((path.to_path_buf(), existed))).ok();
}

/// Turn the job's outcome into the final answer: a stopped job reports
/// `STOPPED` and leaves nothing behind, whatever else went wrong.
pub fn finish<T>(result: Result<T, String>, started: SystemTime) -> Result<T, String> {
    if !is_cancelled() {
        return result;
    }
    let claims = CONTROL.claims.lock().map(|c| c.clone()).unwrap_or_default();
    for (path, existed) in claims {
        sweep(&path, existed, started);
    }
    Err(STOPPED.to_string())
}

/// Remove what the job made. A folder it created goes whole; one it found
/// loses only the files written since the job began.
fn sweep(path: &Path, existed: bool, started: SystemTime) {
    if path.is_dir() {
        if !existed {
            let _ = std::fs::remove_dir_all(path);
            return;
        }
        for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
            let file = entry.path();
            if file.is_file() && written_since(&file, started) {
                let _ = std::fs::remove_file(&file);
            }
        }
    } else if path.is_file() && written_since(path, started) {
        let _ = std::fs::remove_file(path);
    }
}

fn written_since(path: &Path, started: SystemTime) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|modified| modified >= started)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn temp(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("tzz-cancel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn a_folder_the_job_created_is_removed_whole() {
        let root = temp("whole");
        let folder = root.join("New video");
        let started = SystemTime::now();
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("half.mp3"), b"x").unwrap();

        sweep(&folder, false, started);
        assert!(!folder.exists());
    }

    #[test]
    fn a_folder_that_existed_keeps_its_older_files() {
        let root = temp("older");
        let old = root.join("earlier.md");
        std::fs::write(&old, b"kept").unwrap();
        // Filesystems round timestamps; leave a clear gap.
        std::thread::sleep(Duration::from_millis(1100));
        let started = SystemTime::now();
        std::thread::sleep(Duration::from_millis(10));
        let fresh = root.join("half.mp4");
        std::fs::write(&fresh, b"x").unwrap();

        sweep(&root, true, started);
        assert!(old.exists(), "the older file must survive");
        assert!(!fresh.exists(), "the new file must go");
    }

    #[test]
    fn a_stopped_job_reports_stopped_whatever_it_returned() {
        begin();
        cancel();
        let out: Result<u8, String> = finish(Ok(1), SystemTime::now());
        assert_eq!(out, Err(STOPPED.to_string()));
        // Leave the flag down for the next test.
        begin();
    }
}
