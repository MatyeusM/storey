//! Filesystem helpers: canonicalize and sorted dir listing.

use std::path::{Path, PathBuf};

use crate::app::Entry;

/// Canonicalize, falling back to the raw path when it fails.
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Non-hidden children, dirs first then files, case-insensitive sort.
pub fn list_dir(dir: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = item.path();
        let is_dir = path.is_dir();
        out.push(Entry { name, path, is_dir });
    }
    out.sort_by(|a, b| dir_first(a, b).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

fn dir_first(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    b.is_dir.cmp(&a.is_dir)
}

/// Last stderr line, trimmed, for status notices.
pub fn short_err(msg: String) -> String {
    msg.lines().last().unwrap_or("").trim().to_string()
}
