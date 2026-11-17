//! Library root discovery and directory walks.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::index::{self, Record};

/// Returned when no library root exists above start.
pub fn not_found() -> anyhow::Error {
    anyhow::anyhow!("not inside a storey library (no .index.json found)")
}

/// Nearest ancestor of start (including start) containing `.index.json`.
pub fn find_root(start: &Path) -> Result<PathBuf> {
    let mut abs = absolutize(start)?;
    loop {
        if abs.join(index::FILE_NAME).exists() {
            return Ok(abs);
        }
        match abs.parent() {
            Some(parent) if parent != abs => abs = parent.to_path_buf(),
            _ => return Err(not_found()),
        }
    }
}

/// Whether dir itself directly contains `.index.json`.
pub fn has_index_at(dir: &Path) -> bool {
    dir.join(index::FILE_NAME).exists()
}

/// Slash-separated paths (relative to root) of all regular files
/// under root, sorted. Skips `.files/` dirs, every `.index.json`
/// (root or nested — nested libraries merge via adopt, never as
/// content), every symlink, and OS junk; never descends into
/// symlinked directories.
pub fn walk(root: &Path) -> Result<Vec<String>> {
    let mut paths = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let ft = entry.file_type()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if name == index::STORE_DIR {
                    continue;
                }
                stack.push(entry.path());
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            if name == index::FILE_NAME
                || name == index::LOCK_NAME
                || name.starts_with(".index.json.tmp-")
                || ignored_file(&name)
            {
                continue;
            }
            let full = entry.path();
            let rel = full
                .strip_prefix(root)
                .with_context(|| "relativize walk path")?;
            paths.push(slash(rel));
        }
    }
    paths.sort();
    Ok(paths)
}

fn absolutize(p: &Path) -> Result<PathBuf> {
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(p))
}

/// Slash-relative path of `path` inside `root`, or empty.
pub fn slash_rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map(slash).unwrap_or_default()
}

pub(crate) fn slash(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Filesystem path for a slash-relative record path.
pub fn from_slash(rel: &str) -> PathBuf {
    rel.split('/').collect()
}

/// Exact basenames never indexed: OS thumbnails and caches.
const IGNORED_NAMES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini"];
/// Partial-download markers that would freeze half-written files.
const IGNORED_SUFFIXES: &[&str] = &[".part", ".crdownload"];

fn ignored_file(name: &str) -> bool {
    IGNORED_NAMES.contains(&name) || IGNORED_SUFFIXES.iter().any(|s| name.ends_with(s))
}

/// Slash-relative paths of nested libraries: directories below root that
/// contain their own `.index.json`. Root itself is excluded. These are user
/// metadata (possibly shielded by a failed adoption), never view artifacts.
pub fn nested_library_dirs(root: &Path) -> Result<Vec<String>> {
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            if entry.file_name().to_string_lossy() == index::STORE_DIR {
                continue;
            }
            let path = entry.path();
            if has_index_at(&path) {
                dirs.push(slash_rel(root, &path));
            }
            stack.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

/// Every regular file belonging to a nested library. Shielded from fresh
/// scans and from view/restore wipes so a failed adoption can be repaired.
pub fn nested_library_files(root: &Path) -> Result<HashSet<String>> {
    let dirs = nested_library_dirs(root)?;
    if dirs.is_empty() {
        return Ok(HashSet::new());
    }
    Ok(walk(root)?
        .into_iter()
        .filter(|p| dirs.iter().any(|d| p.starts_with(&format!("{d}/"))))
        .collect())
}

/// Sorted slash-relative paths of regular files that are neither at
/// a tracked location nor a known view artifact nor part of a nested library.
pub fn find_untracked(
    root: &Path,
    tracked: &HashSet<String>,
    artifacts: &HashSet<String>,
) -> Result<Vec<String>> {
    let nested = nested_library_files(root)?;
    Ok(walk(root)?
        .into_iter()
        .filter(|p| !tracked.contains(p) && !artifacts.contains(p) && !nested.contains(p))
        .collect())
}

/// Current record paths plus recorded duplicates (stored or in place),
/// slash-relative. Duplicates are known to the index and must not be
/// re-scanned or reported as untracked.
pub fn current_locations(files: &[Record]) -> HashSet<String> {
    let mut out = HashSet::new();
    for rec in files {
        out.insert(rec.location().to_string());
        out.extend(rec.duplicates.iter().cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("storey-core-lib-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join(index::STORE_DIR)).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        std::fs::write(dir.join("sub").join("b.txt"), "b").unwrap();
        std::fs::write(dir.join(index::STORE_DIR).join("c"), "c").unwrap();
        std::fs::write(dir.join(".DS_Store"), "x").unwrap();
        std::fs::write(dir.join("half.part"), "x").unwrap();
        std::fs::write(dir.join(index::FILE_NAME), "{}").unwrap();
        dir
    }

    #[test]
    fn walk_skips_everything() {
        let dir = fixture("walk");
        std::fs::write(dir.join(".index.json.tmp-0-123"), "x").unwrap();
        let paths = walk(&dir).unwrap();
        assert_eq!(paths, vec!["a.txt", "sub/b.txt"]);
        assert!(has_index_at(&dir));
        assert_eq!(find_root(&dir.join("sub")).unwrap(), dir);
        assert!(find_root(&std::env::temp_dir().join("storey-core-nope")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn untracked_subtracts() {
        let dir = fixture("untracked");
        let tracked: HashSet<String> = ["a.txt".to_string()].into_iter().collect();
        let got = find_untracked(&dir, &tracked, &HashSet::new()).unwrap();
        assert_eq!(got, vec!["sub/b.txt"]);
        let locs = current_locations(&[Record {
            duplicates: vec![],
            id: "x".into(),
            display_name: "a.txt".into(),
            original_rel: "a.txt".into(),
            stored_rel: String::new(),
            size: 1,
            tags: Default::default(),
        }]);
        assert!(locs.contains("a.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
