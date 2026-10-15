//! `view` materialization: ingest into `.files/`, wipe
//! non-system entries, rebuild links, record the key order.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::FlowDone;
use crate::index::{Index, STORE_DIR};
use crate::intake;
use crate::library;
use crate::log::Logger;
use crate::materialize_fs;
use crate::views::{self, Link};

/// `view` payload. Counts mirror the human summary.
#[derive(Debug, Serialize)]
pub struct ViewReport {
    pub by: Vec<String>,
    pub moved: usize,
    pub purged: Vec<String>,
    #[serde(default)]
    pub untracked: Vec<String>,
    #[serde(default)]
    pub orphans: Vec<String>,
    pub links: usize,
    pub unsorted: usize,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub link_errors: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub plan: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub moves: BTreeMap<String, String>,
}

/// Untracked files plus view orphans for the block checks.
/// Reconciliation repoints crash residue on the live index.
pub fn scan_blockers(root: &Path, ix: &mut Index) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let orphans = intake::reconcile_store(root, ix);
    let tracked = library::current_locations(&ix.files);
    let artifacts: HashSet<String> = views::plan_set(&ix.files, &ix.view.by)
        .into_iter()
        .collect();
    let untracked = library::find_untracked(root, &tracked, &artifacts)?;
    Ok((untracked, orphans))
}

/// Indexed paths absent from disk.
pub fn missing_locations(root: &Path, ix: &Index) -> Vec<String> {
    ix.files
        .iter()
        .filter(|r| {
            std::fs::symlink_metadata(root.join(library::from_slash(r.location()))).is_err()
        })
        .map(|r| r.location().to_string())
        .collect()
}

/// Current location → store destination (`None` = purge as
/// duplicate), in index order; first hash wins the move.
pub fn compute_moves(ix: &Index) -> BTreeMap<String, Option<String>> {
    let mut taken: HashMap<String, String> = HashMap::new();
    let mut stored_ids: HashSet<String> = HashSet::new();
    for rec in &ix.files {
        if rec.stored() {
            taken.insert(store_base(&rec.stored_rel).to_lowercase(), rec.id.clone());
            stored_ids.insert(rec.id.clone());
        }
    }
    let mut moves: BTreeMap<String, Option<String>> = BTreeMap::new();
    for rec in &ix.files {
        if rec.stored() {
            continue;
        }
        if stored_ids.contains(&rec.id) {
            moves.insert(rec.location().to_string(), None);
            continue;
        }
        let name = views::store_name(&rec.display_name, &taken, &rec.id);
        taken.insert(name.to_lowercase(), rec.id.clone());
        stored_ids.insert(rec.id.clone());
        moves.insert(
            rec.location().to_string(),
            Some(format!("{STORE_DIR}/{name}")),
        );
    }
    moves
}

fn store_base(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

/// Preview post-ingest view links: unstored records with a move
/// destination count as stored, purged dupes are dropped. Empty `by`
/// renders nothing.
pub fn planned_links(
    ix: &Index,
    moves: &BTreeMap<String, Option<String>>,
    by: &[String],
) -> Vec<Link> {
    if by.is_empty() {
        return vec![];
    }
    let mut recs = Vec::with_capacity(ix.files.len());
    for rec in &ix.files {
        let mut rec = rec.clone();
        if let Some(dst) = moves.get(rec.location()) {
            match dst {
                None => continue,
                Some(d) => rec.stored_rel = d.clone(),
            }
        }
        if rec.stored() {
            recs.push(rec);
        }
    }
    views::plan(&recs, by)
}

/// Render planned links as slash-relative paths.
pub fn link_paths(links: &[Link]) -> Vec<String> {
    links.iter().map(|l| l.view_path.clone()).collect()
}

/// Links touching an `Unsorted` segment.
pub fn count_unsorted(links: &[Link]) -> usize {
    links
        .iter()
        .filter(|l| l.view_path.split('/').any(|seg| seg == views::UNSORTED))
        .count()
}

/// Execution context for `exec_view` (keeps arity small).
pub struct ExecCtx<'a> {
    pub root: &'a Path,
    pub by: &'a [String],
    pub untracked: Vec<String>,
    pub orphans: Vec<String>,
    pub moves: &'a BTreeMap<String, Option<String>>,
    pub links: &'a [Link],
    pub logger: &'a Logger,
    pub abort: &'a AtomicBool,
}

/// Execute `view`: probe, move, persist, wipe, link, record.
/// Files have moved before wiping, so the index persists right
/// after the moves — an abort below still leaves a loadable index.
pub fn exec_view(ix: &mut Index, ctx: ExecCtx) -> anyhow::Result<FlowDone<ViewReport>> {
    let ExecCtx {
        root,
        by,
        untracked,
        orphans,
        moves,
        links,
        logger,
        abort,
    } = ctx;
    std::fs::create_dir_all(root.join(STORE_DIR))?;
    materialize_fs::probe_links(root)?;
    if abort.load(Ordering::Relaxed) {
        return Ok(FlowDone::Aborted);
    }
    let (moved, purged) = match materialize_fs::apply_moves(root, ix, moves, abort) {
        Ok(v) => v,
        Err(_) if abort.load(Ordering::Relaxed) => return Ok(FlowDone::Aborted),
        Err(e) => return Err(e),
    };
    ix.save(root)?;
    if abort.load(Ordering::Relaxed) {
        return Ok(FlowDone::Aborted);
    }
    materialize_fs::wipe_non_system(root)?;
    let (made, link_errors) = materialize_fs::create_links(root, ix, links, logger, abort);
    ix.view.by = by.to_vec();
    ix.save(root)?;
    Ok(FlowDone::Done(ViewReport {
        by: by.to_vec(),
        moved,
        purged,
        untracked,
        orphans,
        links: made,
        unsorted: count_unsorted(links),
        link_errors,
        plan: vec![],
        moves: BTreeMap::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Record;

    #[test]
    fn moves_case_variant_names_apart() {
        let mut ix = Index::new();
        for (id, name, size) in [("a", "Song.mp3", 1), ("b", "song.mp3", 2)] {
            ix.files.push(Record {
                id: id.into(),
                display_name: name.into(),
                original_rel: name.into(),
                stored_rel: String::new(),
                size,
                tags: Default::default(),
            });
        }
        let moves = compute_moves(&ix);
        let dsts: Vec<&str> = moves.values().map(|d| d.as_deref().unwrap()).collect();
        assert_eq!(dsts, vec![".files/Song.mp3", ".files/song_1.mp3"]);
    }
}
