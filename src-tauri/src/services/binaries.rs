//! Locating external CLI tools (yt-dlp, ffmpeg, whisper-cli).
//!
//! WHY this module exists: a macOS app launched from Finder does NOT inherit the
//! login shell PATH, so /opt/homebrew/bin is missing and a plain
//! `Command::new("yt-dlp")` fails only in the bundled build -- never in `tauri dev`.
//! We therefore probe well-known install locations before falling back to PATH.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Homebrew (Apple Silicon), Homebrew (Intel), MacPorts, system, pipx/user installs.
const COMMON_BIN_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/opt/local/bin",
    "/usr/bin",
];

#[derive(Debug, Serialize)]
pub struct DependencyStatus {
    pub name: String,
    pub found: bool,
    pub path: Option<String>,
}

/// Resolve an executable by name.
///
/// Order: explicit env override -> known install dirs -> PATH.
/// The env override lets you point at a custom build, e.g. `YT_DLP_PATH=/x/yt-dlp`.
pub fn resolve(name: &str) -> Option<PathBuf> {
    let env_key = format!("{}_PATH", name.to_uppercase().replace('-', "_"));
    if let Ok(custom) = std::env::var(&env_key) {
        let candidate = PathBuf::from(custom);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    for dir in COMMON_BIN_DIRS {
        let candidate = Path::new(dir).join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }

    which(name)
}

/// Same as `resolve`, but returns a user-facing error instead of `None`.
pub fn require(name: &str) -> Result<PathBuf, String> {
    resolve(name).ok_or_else(|| {
        format!("`{name}` not found. Install it with: brew install {name}")
    })
}

pub fn status_for(names: &[&str]) -> Vec<DependencyStatus> {
    names
        .iter()
        .map(|name| {
            let path = resolve(name);
            DependencyStatus {
                name: (*name).to_string(),
                found: path.is_some(),
                path: path.map(|p| p.to_string_lossy().into_owned()),
            }
        })
        .collect()
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return path
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    {
        true
    }
}
