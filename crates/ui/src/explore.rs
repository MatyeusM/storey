//! Explorer directory listing for the GUI.
//!
//! Non-hidden children, dirs first, case-insensitive sort, plus the GUI rule that the content
//! store is never visible: `.files/`, `.index.json`, and every other
//! dotfile are skipped.

use std::path::{Path, PathBuf};

/// One explorer row.
#[derive(Debug, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

/// Canonicalize, falling back to the raw path when it fails.
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// True for names the explorer never shows.
pub fn is_hidden(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    matches!(name, ".files" | ".index.json")
}

/// Listed children of `dir`, store and dotfiles excluded.
pub fn list_dir(dir: &Path) -> Vec<FsEntry> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        if is_hidden(&name) {
            continue;
        }
        let path = item.path();
        let is_dir = path.is_dir();
        let size = if is_dir {
            0
        } else {
            item.metadata().map(|m| m.len()).unwrap_or(0)
        };
        out.push(FsEntry {
            name,
            path,
            is_dir,
            size,
        });
    }
    out.sort_by(|a, b| dir_first(a, b).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

fn dir_first(a: &FsEntry, b: &FsEntry) -> std::cmp::Ordering {
    b.is_dir.cmp(&a.is_dir)
}

/// Last stderr line, trimmed, for status notices.
pub fn short_err(msg: String) -> String {
    msg.lines().last().unwrap_or("").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_store_and_dotfiles() {
        assert!(is_hidden(".files"));
        assert!(is_hidden(".index.json"));
        assert!(is_hidden(".hidden"));
        assert!(!is_hidden("film.mkv"));
    }

    #[test]
    fn lists_sorted_dirs_first() {
        let dir = std::env::temp_dir().join(format!("storey-explore-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bdir")).unwrap();
        std::fs::create_dir_all(dir.join(".files")).unwrap();
        std::fs::write(dir.join("z.txt"), "z").unwrap();
        std::fs::write(dir.join(".index.json"), "{}").unwrap();
        let names: Vec<_> = list_dir(&dir).iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["bdir", "z.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
