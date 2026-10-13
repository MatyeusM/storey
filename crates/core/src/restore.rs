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
    ix.files
        .iter()
        .filter(|r| !r.stored())
        .map(|r| r.original_rel.clone())
        .collect()
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
            if rel == STORE_DIR || rel.starts_with(&format!("{STORE_DIR}/")) {
                continue;
            }
            if rel == FILE_NAME || rel == LOCK_NAME {
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

/// Execute `restore` under the caller's abort flag.
/// Execute `restore` under the caller's abort flag. Reconciles
/// crash residue first (like `view`), so previously stranded
/// store files move back instead of lingering in `.files/`.
pub fn exec_restore(
    root: &Path,
    ix: &mut Index,
    abort: &AtomicBool,
) -> anyhow::Result<FlowDone<usize>> {
    crate::intake::reconcile_store(root, ix);
    let stored = stored_positions(ix);
    clear_views(root, &keep_in_place(ix))?;
    if abort.load(Ordering::Relaxed) {
        return Ok(FlowDone::Aborted);
    }
    match move_back(root, ix, &stored, abort) {
        Ok(restored) => {
            ix.view.by.clear();
            ix.save(root)?;
            Ok(FlowDone::Done(restored))
        }
        Err(_) if abort.load(Ordering::Relaxed) => Ok(FlowDone::Aborted),
        Err(e) => Err(e),
    }
}
