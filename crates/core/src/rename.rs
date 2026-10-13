//! `mv` flow: rename the display name, the file, and all view
//! links. Lossless: nothing is deleted.

use std::path::Path;

use crate::index::{Index, STORE_DIR};
use crate::remove;
use crate::views;

/// `mv` payload: record id plus old and new display names.
#[derive(Debug, serde::Serialize)]
pub struct MvReport {
    pub id: String,
    pub old: String,
    pub new: String,
}

/// The file move for a rename.
pub struct RenamePlan {
    /// Slash-relative current location.
    pub src: String,
    /// Slash-relative destination.
    pub dst: String,
    /// Final display name (after `_N` fallback).
    pub new_name: String,
}

/// Compute the destination. Stored objects stay in `.files/` with
/// `_N` fallback on collision; in-place files keep their directory
/// and abort when the destination exists.
pub fn plan_rename(
    root: &Path,
    ix: &Index,
    pos: usize,
    new_name: &str,
) -> Result<RenamePlan, String> {
    let rec = &ix.files[pos];
    if !rec.stored() {
        let dst = join_dir(parent_of(&rec.original_rel), new_name);
        if std::fs::symlink_metadata(root.join(from_slash(&dst))).is_ok() {
            return Err(format!("mv: {dst} already exists"));
        }
        return Ok(RenamePlan {
            src: rec.location().to_string(),
            dst,
            new_name: new_name.to_string(),
        });
    }
    let mut taken = std::collections::HashMap::new();
    for (i, other) in ix.files.iter().enumerate() {
        if i != pos && other.stored() {
            taken.insert(store_base(&other.stored_rel), other.id.clone());
        }
    }
    let final_name = views::store_name(new_name, &taken, &rec.id);
    Ok(RenamePlan {
        src: rec.location().to_string(),
        dst: format!("{STORE_DIR}/{final_name}"),
        new_name: final_name,
    })
}

fn parent_of(rel: &str) -> &str {
    match rel.rfind('/') {
        Some(i) => &rel[..i],
        None => "",
    }
}

fn join_dir(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

fn store_base(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

fn from_slash(rel: &str) -> std::path::PathBuf {
    rel.split('/').collect()
}

/// Remove old links, move the file, update the record, rebuild
/// links (stored records only).
pub fn apply_rename(
    root: &Path,
    ix: &mut Index,
    pos: usize,
    plan: &RenamePlan,
    old_links: &[String],
) -> anyhow::Result<()> {
    remove::remove_links(root, old_links)?;
    let rec = &mut ix.files[pos];
    let was_stored = rec.stored();
    std::fs::rename(
        root.join(from_slash(&plan.src)),
        root.join(from_slash(&plan.dst)),
    )?;
    let rec = &mut ix.files[pos];
    rec.display_name = plan.new_name.clone();
    if was_stored {
        rec.stored_rel = plan.dst.clone();
    } else {
        rec.original_rel = plan.dst.clone();
    }
    if !ix.files[pos].stored() {
        return Ok(());
    }
    let id = ix.files[pos].id.clone();
    for p in remove::links_for(ix, &id).iter() {
        let abs = root.join(from_slash(p));
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = root.join(from_slash(&ix.files[pos].stored_rel));
        views::create_link(&store, &abs).map_err(|e| anyhow::anyhow!("{p}: {e}"))?;
    }
    Ok(())
}
