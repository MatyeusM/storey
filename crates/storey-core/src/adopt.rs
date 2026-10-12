//! Nested-library adoption: hash-verify sub-indexes record by
//! record, merge them into the parent with tags and names intact,
//! and remove adopted `.index.json` files. Duplicates are purged
//! from disk. Only moves and deletes here — never copies.

use std::path::{Path, PathBuf};

use crate::hash;
use crate::index::{self, Index, Record};
use crate::library;
use crate::log::Logger;

#[path = "adopt_move.rs"]
mod mv;
pub use mv::{adopt_duplicate, adopt_in_place, adopt_stored};

/// Tallies one merge pass.
#[derive(Debug, Default)]
pub struct Result {
    /// Sub-indexes fully adopted and removed.
    pub indexes: usize,
    /// Adopted record ids.
    pub adopted: Vec<String>,
    /// Duplicate ids deleted from disk.
    pub purged: Vec<String>,
    /// Verification failures, left in place.
    pub problems: Vec<String>,
    /// Sub-index dirs with problems (files stay untouched).
    pub skipped: Vec<String>,
}

/// Fold every nested library under root into `ix` (root itself is
/// excluded, as is anything inside `.files/`). A sub-index is
/// deleted only when all of its records adopted cleanly.
pub fn merge(root: &Path, ix: &mut Index, logger: &Logger) -> Result {
    let mut res = Result::default();
    for sub in find_sub_indexes(root) {
        merge_one(root, &sub, ix, logger, &mut res);
    }
    res
}

/// Whether slash-relative path `p` sits inside a skipped (failed)
/// sub-index dir. Shielded files stay untouched so the user can
/// repair the nested library instead of losing its metadata to a
/// fresh, untagged scan.
pub fn shielded(p: &str, skipped: &[String]) -> bool {
    skipped
        .iter()
        .any(|s| p == s || p.starts_with(&format!("{s}/")))
}

fn find_sub_indexes(root: &Path) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if !ft.is_dir() || ft.is_symlink() {
                continue;
            }
            if entry.file_name().to_string_lossy() == index::STORE_DIR {
                continue;
            }
            let p = entry.path();
            if library::has_index_at(&p) {
                dirs.push(slash_rel(root, &p));
            }
            stack.push(p);
        }
    }
    dirs.sort_by(|a, b| depth(b).cmp(&depth(a)).then_with(|| a.cmp(b)));
    dirs
}

fn depth(rel: &str) -> usize {
    rel.chars().filter(|&c| c == '/').count()
}

fn slash_rel(root: &Path, abs: &Path) -> String {
    abs.strip_prefix(root).map(slash).unwrap_or_default()
}

fn slash(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn merge_one(root: &Path, sub: &str, ix: &mut Index, logger: &Logger, res: &mut Result) {
    let sub_abs = root.join(from_slash(sub));
    let sub_ix = match Index::load(&sub_abs) {
        Ok(ix) => ix,
        Err(e) => {
            res.problems.push(format!("{sub}/.index.json: {e}"));
            res.skipped.push(sub.to_string());
            return;
        }
    };
    let by_loc = hash_sub_records(&sub_abs, &sub_ix, logger);
    let mut failed = false;
    for rec in &sub_ix.files {
        if !adopt_record(root, sub, &sub_abs, rec, &by_loc, ix, res) {
            failed = true;
        }
    }
    if failed {
        res.skipped.push(sub.to_string());
        return;
    }
    if let Err(e) = std::fs::remove_file(sub_abs.join(index::FILE_NAME)) {
        res.problems.push(format!("{sub}/.index.json: {e}"));
        return;
    }
    res.indexes += 1;
    let _ = std::fs::remove_dir_all(sub_abs.join(index::STORE_DIR));
    logger.info(format!("adopted nested library {sub}"));
}

fn from_slash(rel: &str) -> PathBuf {
    rel.split('/').collect()
}

/// Hash every indexed location of a sub-library, keyed by
/// slash-relative path inside it.
fn hash_sub_records(
    sub_abs: &Path,
    sub_ix: &Index,
    logger: &Logger,
) -> std::collections::HashMap<String, hash::Outcome> {
    let rels: Vec<String> = sub_ix
        .files
        .iter()
        .map(|r| r.location().to_string())
        .collect();
    let abs: Vec<PathBuf> = rels.iter().map(|r| sub_abs.join(from_slash(r))).collect();
    let out = hash::files(&abs, hash::workers(), |done, total, path| {
        logger.progress("hash", done, total, &crate::intake::rel_of(sub_abs, path));
    });
    out.into_iter()
        .map(|o| {
            let rel = crate::intake::rel_of(sub_abs, &o.path);
            (rel, o)
        })
        .collect()
}

fn adopt_record(
    root: &Path,
    sub: &str,
    sub_abs: &Path,
    rec: &Record,
    by_loc: &std::collections::HashMap<String, hash::Outcome>,
    ix: &mut Index,
    res: &mut Result,
) -> bool {
    if let Some(dup) = ix.find(&rec.id).cloned()
        && moved_earlier(root, rec, &dup)
    {
        return true;
    }
    let outcome = match by_loc.get(rec.location()) {
        Some(o) if o.error.is_none() => o,
        _ => return problem(res, &format!("{sub}/{}: missing", rec.location())),
    };
    if outcome.hash != rec.id {
        return problem(res, &format!("{sub}/{}: content changed", rec.location()));
    }
    if let Some(dup) = ix.find(&rec.id).cloned() {
        return adopt_duplicate(sub, sub_abs, rec, &dup, res);
    }
    if rec.stored() {
        return adopt_stored(root, sub, sub_abs, rec, outcome.size, ix, res);
    }
    let want = format!("{sub}/{}", rec.original_rel);
    if ix.files.iter().any(|r| r.location() == want) {
        return problem(
            res,
            &format!(
                "{sub}/{}: path already indexed with different content",
                rec.location()
            ),
        );
    }
    adopt_in_place(sub, rec, outcome.size, ix, res);
    true
}

/// Whether rec was adopted by a previous pass: a stored record
/// whose content already lives at the parent location.
fn moved_earlier(root: &Path, rec: &Record, dup: &Record) -> bool {
    if !rec.stored() || !dup.stored() {
        return false;
    }
    std::fs::symlink_metadata(root.join(from_slash(&dup.stored_rel))).is_ok()
}

fn problem(res: &mut Result, msg: &str) -> bool {
    res.problems.push(msg.to_string());
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_prefixes() {
        let skipped = vec!["sub".to_string()];
        assert!(shielded("sub", &skipped));
        assert!(shielded("sub/a.txt", &skipped));
        assert!(!shielded("sub2/a.txt", &skipped));
        assert!(!shielded("other", &skipped));
    }

    #[test]
    fn merges_nested_library() {
        let dir = std::env::temp_dir().join(format!("storey-core-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("a.txt"), "hello").unwrap();
        let mut sub_ix = Index::new();
        sub_ix.files.push(Record {
            id: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            display_name: "a.txt".into(),
            original_rel: "a.txt".into(),
            stored_rel: String::new(),
            size: 5,
            tags: [("g".to_string(), vec!["rock".to_string()])]
                .into_iter()
                .collect(),
        });
        sub_ix.save(&sub).unwrap();
        let mut ix = Index::new();
        let logger = Logger::new(false, true);
        let res = merge(&dir, &mut ix, &logger);
        assert!(res.problems.is_empty(), "{:?}", res.problems);
        assert_eq!(res.indexes, 1);
        assert_eq!(ix.files.len(), 1);
        assert_eq!(ix.files[0].original_rel, "sub/a.txt");
        assert!(!sub.join(index::FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
