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
pub fn hash_rels(root: &Path, rels: &[String], logger: &Logger, op: &str) -> Hashed {
    let abs: Vec<std::path::PathBuf> = rels.iter().map(|r| root.join(from_slash(r))).collect();
    let outcomes = hash::files(&abs, hash::workers(), |done, total, path| {
        logger.progress(op, done, total, &rel_of(root, path));
    });
    let mut by_rel = HashMap::with_capacity(outcomes.len());
    let mut errors: HashMap<String, String> = HashMap::new();
    for o in outcomes {
        let rel = rel_of(root, &o.path);
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

fn slash(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Slash-relative path of `path` inside `root`, or empty.
pub(crate) fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).map(slash).unwrap_or_default()
}

fn from_slash(rel: &str) -> std::path::PathBuf {
    rel.split('/').collect()
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
    ordered: &[String],
    by_rel: &HashMap<String, (String, i64)>,
    dry: bool,
    abort: &AtomicBool,
) -> anyhow::Result<Scan> {
    let mut scan = Scan::default();
    let mut by_hash: HashMap<String, usize> = HashMap::with_capacity(ix.files.len());
    for (i, rec) in ix.files.iter().enumerate() {
        by_hash.insert(rec.id.clone(), i);
    }
    for rel in ordered {
        if abort.load(Ordering::Relaxed) {
            anyhow::bail!("interrupted");
        }
        let (hash, size) = &by_rel[rel];
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
                std::fs::remove_file(root.join(from_slash(rel)))?;
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
fn on_disk(root: &Path, rel: &str) -> bool {
    std::fs::metadata(root.join(from_slash(rel))).is_ok()
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
        let abs = root.join(from_slash(&rel));
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

/// Re-hash every indexed path. Changed files are problems; missing
/// files are pruned at the end (renamed or deleted — the entry is
/// dropped, not reported).
pub fn reverify(
    root: &Path,
    ix: &mut Index,
    logger: &Logger,
    abort: &AtomicBool,
) -> (Vec<String>, usize, Vec<String>) {
    let mut problems = Vec::new();
    let mut missing = Vec::new();
    let mut reverified = 0;
    for (i, rec) in ix.files.iter().enumerate() {
        if abort.load(Ordering::Relaxed) {
            break;
        }
        let abs = root.join(from_slash(rec.location()));
        match hash::file(&abs) {
            Err(e) if is_not_found(&e) => missing.push(i),
            Err(e) => problems.push(format!("{}: {e}", rec.location())),
            Ok((h, _)) if h != rec.id => {
                problems.push(format!("{}: content changed", rec.location()))
            }
            Ok(_) => reverified += 1,
        }
        logger.progress("verify", i + 1, ix.files.len(), rec.location());
    }
    let removed = prune_missing(ix, &missing);
    (problems, reverified, removed)
}

fn is_not_found(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<std::io::Error>()
            .map(|io| io.kind() == std::io::ErrorKind::NotFound)
            .unwrap_or(false)
    })
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
mod tests {
    use super::*;

    #[test]
    fn fills_and_dedupes() {
        let dir = std::env::temp_dir().join(format!("storey-core-fill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "same").unwrap();
        std::fs::write(dir.join("b.txt"), "same").unwrap();
        std::fs::write(dir.join("c.txt"), "diff").unwrap();
        let mut ix = Index::new();
        let rels = vec![
            "a.txt".to_string(),
            "b.txt".to_string(),
            "c.txt".to_string(),
        ];
        let logger = Logger::new(false, true);
        let hashed = hash_rels(&dir, &rels, &logger, "hash");
        let abort = AtomicBool::new(false);
        let scan = fill_index(
            &mut ix,
            &dir,
            &hashed.ordered,
            &hashed.by_rel,
            false,
            &abort,
        )
        .unwrap();
        assert_eq!(scan.added.len(), 2);
        assert_eq!(scan.purged.len(), 1);
        assert!(!dir.join("b.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
