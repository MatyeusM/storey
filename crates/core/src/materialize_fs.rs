//! `view` filesystem effects: link probing, wiping, moves, links.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::index::{FILE_NAME, Index, LOCK_NAME, Record, STORE_DIR};
use crate::log::Logger;
use crate::views::{self, Link};

/// Verify the store can host view links before anything moves:
/// symlink preferred, hard link fallback.
pub fn probe_links(root: &Path) -> anyhow::Result<()> {
    let dir = root.join(STORE_DIR);
    let target = dir.join(".probe-target");
    let probe = dir.join(".probe");
    std::fs::write(&target, [])?;
    let ok = symlink_probe(&probe).is_ok() || std::fs::hard_link(&target, &probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    let _ = std::fs::remove_file(&target);
    if ok {
        Ok(())
    } else {
        anyhow::bail!(
            "view links need symlinks or hard links, and this filesystem supports neither \
             (exFAT/FAT32 drives, Windows without developer mode, or a store spanning volumes \
             all fail this way)"
        )
    }
}

#[cfg(unix)]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(".probe-target", probe)
}

#[cfg(windows)]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(".probe-target", probe)
}

#[cfg(not(any(unix, windows)))]
fn symlink_probe(probe: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        probe.display().to_string(),
    ))
}

/// Remove every entry in root except `.files/`, `.index.json`,
/// and the write lock. Symlinks are unlinked, never followed.
pub fn wipe_non_system(root: &Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == STORE_DIR || name == FILE_NAME || name == LOCK_NAME {
            continue;
        }
        let p = root.join(&name);
        let ft = entry.file_type()?;
        if ft.is_dir() && !ft.is_symlink() {
            std::fs::remove_dir_all(&p)?;
        } else {
            std::fs::remove_file(&p)?;
        }
    }
    Ok(())
}

/// Apply the move plan: renames plus duplicate purges.
/// Returns moved count and purged ids.
pub fn apply_moves(
    root: &Path,
    ix: &mut Index,
    moves: &BTreeMap<String, Option<String>>,
    abort: &AtomicBool,
) -> anyhow::Result<(usize, Vec<String>)> {
    let mut moved = 0;
    let mut purged = Vec::new();
    for rec in ix.files.iter_mut() {
        if abort.load(Ordering::Relaxed) {
            anyhow::bail!("interrupted");
        }
        let dst = match moves.get(rec.location()) {
            Some(dst) => dst.clone(),
            None => continue,
        };
        let src = root.join(from_slash(rec.location()));
        match dst {
            None => {
                std::fs::remove_file(&src)?;
                purged.push(rec.id.clone());
            }
            Some(dst) => {
                std::fs::rename(&src, root.join(from_slash(&dst)))?;
                rec.stored_rel = dst;
                moved += 1;
            }
        }
    }
    Ok((moved, purged))
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
        let abs = root.join(from_slash(&link.view_path));
        if let Some(parent) = abs.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{}: {e}", link.view_path));
            logger.warn(format!("view: {}: {e}", link.view_path));
            continue;
        }
        let store = root.join(from_slash(&rec.stored_rel));
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

fn from_slash(rel: &str) -> std::path::PathBuf {
    rel.split('/').collect()
}
