//! `restore` flow: remove view artifacts and move stored
//! objects back to their original relative paths.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::FlowDone;
use crate::index::{FILE_NAME, Index, LOCK_NAME, STORE_DIR};
use crate::library;

/// `restore` payload.
#[derive(Debug, serde::Serialize)]
pub struct RestoreReport {
    pub restored: usize,
    pub skipped: usize,
    pub conflicts: Option<Vec<String>>,
    pub problems: Vec<String>,
    pub untracked: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub moves: BTreeMap<String, String>,
    /// Duplicate files recreated from their canonical object (--duplicates).
    pub duplicates_recreated: usize,
    /// Duplicate paths to be recreated (dry run) or recreated (live).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub duplicate_paths: Vec<String>,
}

/// Outcome of a live restore: canonical files moved back, duplicates recreated.
#[derive(Debug, Default)]
pub struct RestoreOutcome {
    pub restored: usize,
    pub duplicates: usize,
    pub duplicate_paths: Vec<String>,
    pub failures: Vec<String>,
}

/// Positions of records already ingested into `.files/`.
pub fn stored_positions(ix: &Index) -> Vec<usize> {
    ix.files
        .iter()
        .enumerate()
        .filter(|(_, r)| r.stored())
        .map(|(i, _)| i)
        .collect()
}

/// Original paths shadowed by on-disk files.
pub fn find_conflicts(root: &Path, ix: &Index, stored: &[usize]) -> Vec<String> {
    stored
        .iter()
        .filter(|&&i| {
            std::fs::symlink_metadata(root.join(library::from_slash(&ix.files[i].original_rel)))
                .is_ok()
        })
        .map(|&i| ix.files[i].original_rel.clone())
        .collect()
}

/// Stored objects missing from disk.
pub fn missing_stored(root: &Path, ix: &Index, stored: &[usize]) -> Vec<String> {
    stored
        .iter()
        .filter(|&&i| {
            std::fs::symlink_metadata(root.join(library::from_slash(&ix.files[i].stored_rel)))
                .is_err()
        })
        .map(|&i| ix.files[i].stored_rel.clone())
        .collect()
}

/// Store-to-original moves for preview and reporting.
pub fn restore_moves(ix: &Index, stored: &[usize]) -> BTreeMap<String, String> {
    stored
        .iter()
        .map(|&i| {
            (
                ix.files[i].stored_rel.clone(),
                ix.files[i].original_rel.clone(),
            )
        })
        .collect()
}

/// Original paths of records never ingested (survive the wipe).
pub fn keep_in_place(ix: &Index) -> HashSet<String> {
    let mut keep: HashSet<String> = ix
        .files
        .iter()
        .filter(|r| !r.stored())
        .map(|r| r.original_rel.clone())
        .collect();
    keep.extend(ix.files.iter().flat_map(|r| r.duplicates.iter().cloned()));
    keep
}

/// Move stored objects back, clearing their store pointers.
/// A destination appearing mid-run aborts with an error (the
/// payload is not emitted then).
pub fn move_back(
    root: &Path,
    ix: &mut Index,
    stored: &[usize],
    abort: &AtomicBool,
) -> anyhow::Result<usize> {
    let mut restored = 0;
    for &i in stored {
        if abort.load(Ordering::Relaxed) {
            anyhow::bail!("interrupted");
        }
        let (src, dst, orig) = (
            ix.files[i].stored_rel.clone(),
            ix.files[i].original_rel.clone(),
            ix.files[i].original_rel.clone(),
        );
        if std::fs::symlink_metadata(root.join(library::from_slash(&dst))).is_ok() {
            anyhow::bail!("restore: {orig} appeared during restore");
        }
        if let Some(parent) = root.join(library::from_slash(&dst)).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(
            root.join(library::from_slash(&src)),
            root.join(library::from_slash(&dst)),
        )?;
        ix.files[i].stored_rel.clear();
        // Persist each completed move so a later failure leaves the index
        // consistent with disk instead of pointing at a missing store blob.
        ix.save(root)?;
        restored += 1;
    }
    Ok(restored)
}

/// Remove every non-system entry except kept files. Symlinks are
/// unlinked, never followed.
pub fn clear_views(root: &Path, keep: &HashSet<String>) -> anyhow::Result<()> {
    let (files, dirs) = collect_entries(root, keep)?;
    for f in &files {
        let abs = root.join(library::from_slash(f));
        let ft = std::fs::symlink_metadata(&abs)?.file_type();
        if ft.is_dir() && !ft.is_symlink() {
            std::fs::remove_dir_all(&abs)?;
        } else {
            std::fs::remove_file(&abs)?;
        }
    }
    remove_empty_dirs(root, &dirs, &keep_dir_set(keep))
}

fn collect_entries(
    root: &Path,
    keep: &HashSet<String>,
) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let rel = library::slash_rel(root, &entry.path());
            if rel.split('/').any(|part| part == STORE_DIR) {
                continue;
            }
            let name = rel.rsplit('/').next().unwrap_or(&rel);
            if name == FILE_NAME || name == LOCK_NAME {
                continue;
            }
            // Symlinks (and plain files) are entries; real dirs
            // descend. NOTE: never skip siblings here.
            if ft.is_dir() && !ft.is_symlink() {
                dirs.push(rel);
                stack.push(entry.path());
                continue;
            }
            if keep.contains(&rel) {
                continue;
            }
            files.push(rel);
        }
    }
    Ok((files, dirs))
}

fn keep_dir_set(keep: &HashSet<String>) -> HashSet<String> {
    let mut set = HashSet::from([String::new()]);
    for p in keep {
        let mut dir = p.as_str();
        while let Some(i) = dir.rfind('/') {
            dir = &dir[..i];
            set.insert(dir.to_string());
        }
    }
    set
}

fn remove_empty_dirs(
    root: &Path,
    dirs: &[String],
    keep_dirs: &HashSet<String>,
) -> anyhow::Result<()> {
    let mut ordered = dirs.to_vec();
    ordered.sort_by_key(|d| std::cmp::Reverse(d.len()));
    for d in &ordered {
        if keep_dirs.contains(d) {
            continue;
        }
        let abs = root.join(library::from_slash(d));
        match std::fs::remove_dir(&abs) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                let non_empty = std::fs::read_dir(&abs)
                    .map(|mut r| r.next().is_some())
                    .unwrap_or(true);
                if non_empty {
                    anyhow::bail!("restore: cannot clear {d}: {e}");
                }
            }
        }
    }
    Ok(())
}

/// Recreate duplicate files from their canonical object. A duplicate has the
/// same content hash, so a hard link is enough — no bytes are copied; a
/// symlink is the fallback when the filesystem refuses hard links. Paths are
/// validated on index load, so they cannot escape the library root.
/// Execute `restore` under the caller's abort flag. Reconciles
/// crash residue first (like `view`), so previously stranded
/// store files move back instead of lingering in `.files/`.
/// With `duplicates`, also recreates the files listed in each record's
/// `duplicates` array (view removed them as redundant copies).
pub fn exec_restore(
    root: &Path,
    ix: &mut Index,
    abort: &AtomicBool,
    duplicates: bool,
) -> anyhow::Result<FlowDone<RestoreOutcome>> {
    crate::intake::reconcile_store(root, ix);
    let stored = stored_positions(ix);
    // Persist the reconciled state before any destructive clear so a crash
    // or failure cannot leave the index pointing at files we just removed.
    ix.save(root)?;
    let mut keep = keep_in_place(ix);
    keep.extend(library::nested_library_files(root)?);
    clear_views(root, &keep)?;
    if abort.load(Ordering::Relaxed) {
        return Ok(FlowDone::Aborted);
    }
    let restored = match move_back(root, ix, &stored, abort) {
        Ok(n) => n,
        Err(_) if abort.load(Ordering::Relaxed) => return Ok(FlowDone::Aborted),
        Err(e) => return Err(e),
    };
    ix.view.by.clear();
    let mut outcome = RestoreOutcome {
        restored,
        ..RestoreOutcome::default()
    };
    if duplicates {
        outcome = recreate_duplicates(root, ix);
        outcome.restored = restored;
    }
    ix.save(root)?;
    Ok(FlowDone::Done(outcome))
}

pub use crate::restore_dups::{planned_duplicates, recreate_duplicates};
