//! `mv` flow: rename the display name, the file, and all view
//! links. Lossless: nothing is deleted.

use std::path::Path;

use crate::index::{Index, STORE_DIR};
use crate::library;
use crate::log::Logger;
use crate::remove;
use crate::views;

/// `mv` payload: record id plus old and new display names.
#[derive(Debug, serde::Serialize)]
pub struct MvReport {
    pub id: String,
    pub old: String,
    pub new: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub link_errors: Vec<String>,
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
        if std::fs::symlink_metadata(root.join(library::from_slash(&dst))).is_ok() {
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
            taken.insert(
                store_base(&other.stored_rel).to_lowercase(),
                other.id.clone(),
            );
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

/// Context for `apply_rename` (keeps the function's arity small).
pub struct RenameCtx<'a> {
    pub root: &'a Path,
    pub pos: usize,
    pub plan: &'a RenamePlan,
    pub old_links: &'a [String],
    pub logger: &'a Logger,
}

/// Remove old links, move the file, update the record, then rebuild every
/// link. The index is saved right after the file move and record update, so a
/// later link failure can never leave an unsaved, inconsistent index that a
/// subsequent `--verify` would prune (taking the record's tags with it).
/// Link failures are warnings, not errors — exactly like `view`.
pub fn apply_rename(ix: &mut Index, ctx: RenameCtx) -> anyhow::Result<Vec<String>> {
    let RenameCtx {
        root,
        pos,
        plan,
        old_links,
        logger,
    } = ctx;
    remove::remove_links(root, old_links)?;
    let was_stored = ix.files[pos].stored();
    std::fs::rename(
        root.join(library::from_slash(&plan.src)),
        root.join(library::from_slash(&plan.dst)),
    )?;
    let rec = &mut ix.files[pos];
    rec.display_name = plan.new_name.clone();
    if was_stored {
        rec.stored_rel = plan.dst.clone();
    } else {
        rec.original_rel = plan.dst.clone();
    }
    ix.save(root)?;
    if !ix.files[pos].stored() {
        return Ok(vec![]);
    }
    let stored: Vec<_> = ix.files.iter().filter(|r| r.stored()).cloned().collect();
    let new_links = crate::views::plan(&stored, &ix.view.by);
    let by_id: std::collections::HashMap<&str, &crate::index::Record> =
        ix.files.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut failed = Vec::new();
    for link in new_links {
        let abs = root.join(library::from_slash(&link.view_path));
        if let Some(parent) = abs.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            failed.push(format!("{}: {e}", link.view_path));
            logger.warn(format!("mv: {}: {e}", link.view_path));
            continue;
        }
        let Some(src) = by_id.get(link.id.as_str()).map(|r| r.stored_rel.clone()) else {
            continue;
        };
        let store = root.join(library::from_slash(&src));
        if let Err(e) = views::create_link(&store, &abs) {
            failed.push(format!("{}: {e}", link.view_path));
            logger.warn(format!("mv: cannot link {}: {e}", link.view_path));
        }
    }
    Ok(failed)
}
