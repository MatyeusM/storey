//! Intake mechanics shared by `init` and `update`: hashing
//! candidates, index filling (record/repoint/purge), store
//! reconciliation, and trusted re-verification.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::adopt;
use crate::hash;
use crate::index::{Index, Record};
use crate::library;
use crate::log::Logger;

/// One hashing batch: content keyed by slash-relative path, walk
/// order preserved, unreadable files reported (never fatal).
pub struct Hashed {
    pub by_rel: HashMap<String, (String, i64)>,
    pub ordered: Vec<String>,
    pub problems: Vec<String>,
}

/// Walk-ordered hashing of slash-relative paths under root,
/// reporting progress as `op` through the logger.
pub fn hash_rels(
    root: &Path,
    rels: &[String],
    logger: &Logger,
    op: &str,
    abort: &AtomicBool,
) -> Hashed {
    let abs: Vec<std::path::PathBuf> = rels
        .iter()
        .map(|r| root.join(library::from_slash(r)))
        .collect();
    let outcomes = hash::files(&abs, hash::workers(), abort, |done, total, path| {
        logger.progress(op, done, total, &library::slash_rel(root, path));
    });
    let mut by_rel = HashMap::with_capacity(outcomes.len());
    let mut errors: HashMap<String, String> = HashMap::new();
    for o in outcomes {
        let rel = library::slash_rel(root, &o.path);
        match o.error {
            Some(msg) => {
                errors.insert(rel, msg);
            }
            None => {
                by_rel.insert(rel, (o.hash, o.size));
            }
        }
    }
    let mut ordered = Vec::new();
    let mut problems = Vec::new();
    for rel in rels {
        if by_rel.contains_key(rel) {
            ordered.push(rel.clone());
        } else if let Some(msg) = errors.get(rel) {
            problems.push(format!("{rel}: {msg}"));
        } else {
            problems.push(format!("{rel}: unreadable"));
        }
    }
    Hashed {
        by_rel,
        ordered,
        problems,
    }
}

/// Files needing hashing: not at a tracked location, not a derived
/// view artifact, and not under a failed nested library.
pub fn candidates(root: &Path, ix: &Index, skipped: &[String]) -> anyhow::Result<Vec<String>> {
    let all = library::walk(root)?;
    let known = library::current_locations(&ix.files);
    let artifacts = crate::views::plan_set(&ix.files, &ix.view.by);
    Ok(all
        .into_iter()
        .filter(|p| !known.contains(p) && !artifacts.contains(p) && !adopt::shielded(p, skipped))
        .collect())
}

/// Scan outcome: added ids, purged duplicate hashes, repointed ids.
#[derive(Debug, Default)]
pub struct Scan {
    pub added: Vec<String>,
    pub purged: Vec<String>,
    pub moved: Vec<String>,
}

/// Record hashed paths in walk order; first hash wins. A later path
/// with a known hash is a duplicate and deleted from disk — unless
/// the indexed file is gone, which means the content moved: repoint
/// the record (keeping tags) instead. Under `dry`, nothing is
/// deleted or repointed: classification only.
pub fn fill_index(
    ix: &mut Index,
    root: &Path,
    hashed: &Hashed,
    dry: bool,
    abort: &AtomicBool,
) -> anyhow::Result<Scan> {
    let mut scan = Scan::default();
    let mut by_hash: HashMap<String, usize> = HashMap::with_capacity(ix.files.len());
    for (i, rec) in ix.files.iter().enumerate() {
        by_hash.insert(rec.id.clone(), i);
    }
    for rel in &hashed.ordered {
        if abort.load(Ordering::Relaxed) {
            anyhow::bail!("interrupted");
        }
        let (hash, size) = &hashed.by_rel[rel];
        if let Some(&j) = by_hash.get(hash) {
            if !on_disk(root, ix.files[j].location()) {
                let dup = &mut ix.files[j];
                dup.original_rel = rel.clone();
                dup.display_name = basename(rel);
                dup.stored_rel.clear();
                dup.size = *size;
                scan.moved.push(dup.id.clone());
                continue;
            }
            if !dry {
                std::fs::remove_file(root.join(library::from_slash(rel)))?;
            }
            scan.purged.push(hash.clone());
            continue;
        }
        ix.files.push(Record {
            id: hash.clone(),
            display_name: basename(rel),
            original_rel: rel.clone(),
            stored_rel: String::new(),
            size: *size,
            tags: Default::default(),
        });
        by_hash.insert(hash.clone(), ix.files.len() - 1);
        scan.added.push(hash.clone());
    }
    Ok(scan)
}

fn basename(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

/// Whether rel exists under root. Any error counts as gone (never
/// delete on doubt): a dangling link repoints rather than purges.
pub(crate) fn on_disk(root: &Path, rel: &str) -> bool {
    std::fs::metadata(root.join(library::from_slash(rel))).is_ok()
}

/// Heal crash residue: files sitting in `.files/` with no record
/// pointing at them. A hash match against a record whose location is
/// gone repoints it (tags kept); anything else is reported.
pub fn reconcile_store(root: &Path, ix: &mut Index) -> Vec<String> {
    let referenced: HashSet<String> = ix
        .files
        .iter()
        .filter(|r| r.stored())
        .map(|r| r.stored_rel.clone())
        .collect();
    let store = root.join(crate::index::STORE_DIR);
    let entries = match std::fs::read_dir(&store) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let by_hash: HashMap<String, usize> = ix
        .files
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id.clone(), i))
        .collect();
    let mut orphans = Vec::new();
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(true) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{}/{name}", crate::index::STORE_DIR);
        if referenced.contains(&rel) {
            continue;
        }
        let abs = root.join(library::from_slash(&rel));
        let (hash, size) = match hash::file(&abs) {
            Ok(v) => v,
            Err(e) => {
                orphans.push(format!("{rel}: {e}"));
                continue;
            }
        };
        match by_hash.get(&hash) {
            Some(&j) if !on_disk(root, ix.files[j].location()) => {
                let rec = &mut ix.files[j];
                rec.stored_rel = rel;
                rec.size = size;
            }
            _ => orphans.push(rel),
        }
    }
    orphans
}

/// Re-hash every indexed path through the worker pool. Changed
/// files are problems; missing files are pruned at the end (renamed
/// or deleted — the entry is dropped, not reported). Verdicts are
/// folded in index order, so reports stay deterministic.
pub fn reverify(
    root: &Path,
    ix: &mut Index,
    logger: &Logger,
    abort: &AtomicBool,
) -> (Vec<String>, usize, Vec<String>) {
    let rels: Vec<String> = ix.files.iter().map(|r| r.location().to_string()).collect();
    let abs: Vec<std::path::PathBuf> = rels
        .iter()
        .map(|r| root.join(library::from_slash(r)))
        .collect();
    let outcomes = hash::files(&abs, hash::workers(), abort, |done, total, path| {
        logger.progress("verify", done, total, &library::slash_rel(root, path));
    });
    let by_rel: HashMap<String, hash::Outcome> = outcomes
        .into_iter()
        .map(|o| (library::slash_rel(root, &o.path), o))
        .collect();
    let mut problems = Vec::new();
    let mut missing = Vec::new();
    let mut reverified = 0;
    for (i, rec) in ix.files.iter().enumerate() {
        let Some(o) = by_rel.get(&rels[i]) else {
            continue; // aborted before this file was hashed
        };
        if o.missing {
            missing.push(i);
        } else if let Some(e) = &o.error {
            problems.push(format!("{}: {e}", rels[i]));
        } else if o.hash != rec.id {
            problems.push(format!("{}: content changed", rels[i]));
        } else {
            reverified += 1;
        }
    }
    let removed = prune_missing(ix, &missing);
    (problems, reverified, removed)
}

fn prune_missing(ix: &mut Index, missing: &[usize]) -> Vec<String> {
    let removed: Vec<String> = missing.iter().map(|&i| ix.files[i].id.clone()).collect();
    for &i in missing.iter().rev() {
        ix.files.remove(i);
    }
    removed
}

/// Throwaway copy for dry-run simulation.
pub fn copy_index(ix: &Index) -> Index {
    ix.clone()
}

#[cfg(test)]
#[path = "intake_tests.rs"]
mod tests;
