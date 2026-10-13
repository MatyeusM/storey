//! `rm` flow plus shared view-link removal: delete an object's
//! view links, then its file and index record. Permanent.

use std::path::Path;

use crate::index::Index;

/// `rm` payload: the removed record id.
#[derive(Debug, serde::Serialize)]
pub struct RmReport {
    pub removed: String,
}

/// Current view link paths for one id (empty when no view was
/// ever materialized, so plain filenames are never misclassified).
pub fn links_for(ix: &Index, id: &str) -> Vec<String> {
    if ix.view.by.is_empty() {
        return vec![];
    }
    let stored: Vec<_> = ix.files.iter().filter(|r| r.stored()).cloned().collect();
    crate::views::plan(&stored, &ix.view.by)
        .into_iter()
        .filter(|l| l.id == id)
        .map(|l| l.view_path)
        .collect()
}

/// Unlink view paths, pruning newly-empty parent dirs.
pub fn remove_links(root: &Path, links: &[String]) -> std::io::Result<()> {
    for p in links {
        let abs = root.join(from_slash(p));
        match std::fs::remove_file(&abs) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        prune_parents(root, p);
    }
    Ok(())
}

fn prune_parents(root: &Path, rel: &str) {
    let mut dir = parent_dir(rel);
    while !dir.is_empty() && dir != "." {
        if std::fs::remove_dir(root.join(from_slash(dir))).is_err() {
            return;
        }
        dir = parent_dir(dir);
    }
}

fn parent_dir(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

fn from_slash(rel: &str) -> std::path::PathBuf {
    rel.split('/').collect()
}

/// Delete the record's links and file, dropping it from the index.
/// Returns the report; the caller saves.
pub fn exec_rm(root: &Path, ix: &mut Index, pos: usize) -> anyhow::Result<RmReport> {
    let id = ix.files[pos].id.clone();
    let location = ix.files[pos].location().to_string();
    let links = links_for(ix, &id);
    remove_links(root, &links)?;
    let abs = root.join(from_slash(&location));
    if let Err(e) = std::fs::symlink_metadata(&abs) {
        anyhow::bail!("rm: {location}: {e}");
    }
    std::fs::remove_file(&abs)?;
    ix.files.remove(pos);
    Ok(RmReport { removed: id })
}
