//! `view` filesystem effects: link probing, wiping, moves, links.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::index::{FILE_NAME, Index, LOCK_NAME, Record, STORE_DIR};
use crate::library;
use crate::log::Logger;
use crate::views::{self, Link};

/// Verify the store can host view links before anything moves:
/// symlink preferred, hard link fallback.
///
/// The probe lives in its own uniquely-named directory inside `.files/` so
/// it can never collide with — let alone truncate — a stored object (a file
/// legitimately named `.probe-target` used to be destroyed on every view).
pub fn probe_links(root: &Path) -> anyhow::Result<()> {
    let stamp = format!(
        ".probe-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let dir = root.join(STORE_DIR);
    let mut probe_dir = None;
    for i in 0..100 {
        let candidate = dir.join(format!("{stamp}-{i}"));
        match std::fs::create_dir(&candidate) {
            Ok(()) => {
                probe_dir = Some(candidate);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    let probe_dir = probe_dir.ok_or_else(|| anyhow::anyhow!("no free probe name in {dir:?}"))?;
    let result = (|| {
        let target = probe_dir.join("target");
        std::fs::write(&target, [])?;
        let link = probe_dir.join("link");
        let ok = symlink_probe(&link).is_ok() || std::fs::hard_link(&target, &link).is_ok();
        if ok {
            Ok(())
        } else {
            anyhow::bail!(
                "view links need symlinks or hard links, and this filesystem supports neither \
                 (exFAT/FAT32 drives, Windows without developer mode, or a store spanning volumes \
                 all fail this way)"
            )
        }
    })();
    let _ = std::fs::remove_dir_all(&probe_dir);
    result
}

#[cfg(unix)]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink("target", probe)
}

#[cfg(windows)]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file("target", probe)
}

#[cfg(not(any(unix, windows)))]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        probe.display().to_string(),
    ))
}

/// Remove every regenerable entry in root except the store, the index,
/// the write lock, recorded duplicates and nested libraries. Symlinks are
/// unlinked, never followed. `keep` holds slash-relative paths that must
/// survive the wipe; their parent directories are kept too.
pub fn wipe_non_system(root: &Path, keep: &HashSet<String>) -> anyhow::Result<()> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let rel = library::slash_rel(root, &entry.path());
            if rel
                .split('/')
                .any(|part| part == STORE_DIR || part == FILE_NAME || part == LOCK_NAME)
            {
                continue;
            }
            if ft.is_dir() && !ft.is_symlink() {
                dirs.push(rel);
                stack.push(entry.path());
            } else {
                files.push(rel);
            }
        }
    }
    let ancestors = ancestor_dirs(keep);
    for rel in files {
        if keep.contains(&rel) {
            continue;
        }
        std::fs::remove_file(root.join(library::from_slash(&rel)))?;
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
    for rel in dirs {
        if keep.contains(&rel) || ancestors.contains(&rel) {
            continue;
        }
        match std::fs::remove_dir(root.join(library::from_slash(&rel))) {
            Ok(()) => {}
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    || e.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// All parent directories of the kept paths (so `a/b/file` keeps `a` and `a/b`).
fn ancestor_dirs(paths: &HashSet<String>) -> HashSet<String> {
    let mut out = HashSet::new();
    for path in paths {
        let mut parts: Vec<&str> = path.split('/').collect();
        while parts.pop().is_some() {
            if parts.is_empty() {
                break;
            }
            out.insert(parts.join("/"));
        }
    }
    out
}

/// Apply the move plan: renames plus non-destructive duplicate recording.
/// A duplicate file stays on disk and is appended to the canonical record's
/// `duplicates`; a legacy duplicate record is folded into the canonical one.
/// Returns (moved count, duplicate paths kept).
pub fn apply_moves(
    root: &Path,
    ix: &mut Index,
    moves: &BTreeMap<String, Option<String>>,
    abort: &AtomicBool,
) -> anyhow::Result<(usize, Vec<String>)> {
    let mut canonical: HashMap<String, usize> = HashMap::new();
    for (i, rec) in ix.files.iter().enumerate() {
        match canonical.get(&rec.id) {
            None => {
                canonical.insert(rec.id.clone(), i);
            }
            Some(&prev) if !ix.files[prev].stored() && rec.stored() => {
                canonical.insert(rec.id.clone(), i);
            }
            _ => {}
        }
    }
    let mut moved = 0;
    let mut duplicates = Vec::new();
    let mut add = Vec::new();
    let mut drop = Vec::new();
    for i in 0..ix.files.len() {
        if abort.load(Ordering::Relaxed) {
            anyhow::bail!("interrupted");
        }
        let loc = ix.files[i].location().to_string();
        let dst = match moves.get(&loc) {
            Some(dst) => dst.clone(),
            None => continue,
        };
        match dst {
            None => {
                duplicates.push(loc.clone());
                let owner = canonical.get(&ix.files[i].id).copied().unwrap_or(i);
                if owner != i {
                    add.push((owner, loc));
                }
                drop.push(i);
            }
            Some(dst) => {
                let src = root.join(library::from_slash(&loc));
                std::fs::rename(&src, root.join(library::from_slash(&dst)))?;
                ix.files[i].stored_rel = dst;
                moved += 1;
            }
        }
    }
    for (owner, loc) in add {
        ix.files[owner].add_duplicate(loc);
    }
    for i in drop.into_iter().rev() {
        ix.files.remove(i);
    }
    Ok((moved, duplicates))
}

/// Build every planned link; failures are warnings, never fatal.
pub fn create_links(
    root: &Path,
    ix: &Index,
    links: &[Link],
    logger: &Logger,
    abort: &AtomicBool,
) -> (usize, Vec<String>) {
    let mut made = 0;
    let mut failed = Vec::new();
    let by_id: HashMap<&str, &Record> = ix.files.iter().map(|r| (r.id.as_str(), r)).collect();
    for link in links {
        if abort.load(Ordering::Relaxed) {
            break;
        }
        let Some(rec) = by_id.get(link.id.as_str()) else {
            failed.push(format!("{}: unknown id {}", link.view_path, link.id));
            logger.warn(format!("view: {}: unknown id {}", link.view_path, link.id));
            continue;
        };
        let abs = root.join(library::from_slash(&link.view_path));
        if let Some(parent) = abs.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{}: {e}", link.view_path));
            logger.warn(format!("view: {}: {e}", link.view_path));
            continue;
        }
        let store = root.join(library::from_slash(&rec.stored_rel));
        match views::create_link(&store, &abs) {
            Ok(_) => made += 1,
            Err(e) => {
                failed.push(format!("{}: {e}", link.view_path));
                logger.warn(format!("view: cannot link {}: {e}", link.view_path));
            }
        }
    }
    (made, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("storey-core-mfs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(STORE_DIR)).unwrap();
        dir
    }

    #[test]
    fn probe_never_touches_a_stored_probe_target() {
        let dir = fixture("probe");
        // A legitimate stored object with the old probe's name.
        std::fs::write(dir.join(STORE_DIR).join(".probe-target"), b"precious").unwrap();
        std::fs::write(dir.join(STORE_DIR).join(".probe"), b"also precious").unwrap();
        probe_links(&dir).unwrap();
        assert_eq!(
            std::fs::read(dir.join(STORE_DIR).join(".probe-target")).unwrap(),
            b"precious"
        );
        assert_eq!(
            std::fs::read(dir.join(STORE_DIR).join(".probe")).unwrap(),
            b"also precious"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wipe_preserves_kept_duplicates_and_nested_metadata() {
        let dir = fixture("wipe");
        std::fs::write(dir.join("view-link.txt"), "link").unwrap();
        std::fs::write(dir.join("copy.txt"), "dup").unwrap();
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        std::fs::write(dir.join("nested/.index.json"), "{}").unwrap();
        std::fs::write(dir.join("nested/tag.txt"), "keep").unwrap();
        let keep = HashSet::from(["copy.txt".to_string(), "nested/tag.txt".to_string()]);
        wipe_non_system(&dir, &keep).unwrap();
        assert!(dir.join("copy.txt").exists());
        assert!(dir.join("nested/.index.json").exists());
        assert!(dir.join("nested/tag.txt").exists());
        assert!(!dir.join("view-link.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
