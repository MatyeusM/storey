//! Side-pane quick places plus human sizes and explorer open.
//!
//! [`quick_places`] anchors at `$HOME` (or `%USERPROFILE%`) and
//! keeps only directories that exist, so the side pane never offers
//! dead entries. [`human_size`] renders the status-bar byte totals.

use std::path::{Path, PathBuf};

/// One quick-access place for the side pane.
#[derive(Debug, Clone)]
pub struct Place {
    pub name: String,
    pub path: PathBuf,
}

/// Home-anchored quick places that exist, plus the filesystem root.
pub(crate) fn quick_places() -> Vec<Place> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from));
    let mut out = Vec::new();
    if let Some(home) = home {
        push_if_dir(&mut out, "Home", &home);
        for name in [
            "Desktop",
            "Documents",
            "Downloads",
            "Music",
            "Pictures",
            "Videos",
        ] {
            push_if_dir(&mut out, name, &home.join(name));
        }
    }
    #[cfg(windows)]
    push_if_dir(&mut out, "This PC", Path::new("C:\\"));
    #[cfg(not(windows))]
    push_if_dir(&mut out, "File System", Path::new("/"));
    out
}

/// Push a place when its path is a directory (dedupes the root).
fn push_if_dir(out: &mut Vec<Place>, name: &str, path: &Path) {
    if !path.is_dir() {
        return;
    }
    if out.iter().any(|p| p.path == path) {
        return;
    }
    out.push(Place {
        name: name.to_string(),
        path: path.to_path_buf(),
    });
}

/// Places serialized for the side pane.
pub(crate) fn places_json(places: &[Place]) -> String {
    let rows: Vec<serde_json::Value> = places
        .iter()
        .map(|p| {
            serde_json::json!({
                "name": p.name,
                "path": p.path.display().to_string(),
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// 1024-based size (`12 B`, `1.5 kB`, `3.2 MB`, `1.1 GB`, `2.0 TB`).
pub(crate) fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < 4 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Open `dir` in the OS file explorer (detached, best effort).
pub(crate) fn open_in_explorer(dir: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let (bin, arg) = ("explorer", dir.display().to_string());
    #[cfg(target_os = "macos")]
    let (bin, arg) = ("open", dir.display().to_string());
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let (bin, arg) = ("xdg-open", dir.display().to_string());
    std::process::Command::new(bin)
        .arg(arg)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_binary_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 kB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn places_serialize() {
        let places = vec![Place {
            name: "Home".into(),
            path: PathBuf::from("/home/u"),
        }];
        let rows: serde_json::Value = serde_json::from_str(&places_json(&places)).unwrap();
        assert_eq!(rows[0]["name"], "Home");
    }
}
