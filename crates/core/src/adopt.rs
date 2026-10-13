//! Nested-library adoption: hash-verify sub-indexes record by
//! record, merge them into the parent with tags and names intact,
//! and remove adopted `.index.json` files. Duplicates are purged
//! from disk. Only moves and deletes here — never copies.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::FlowDone;
use crate::hash;
use crate::index::{self, Index, Record};
use crate::library::{self, slash_rel};
use crate::log::Logger;

#[path = "adopt_move.rs"]
mod mv;
pub(crate) use mv::{adopt_duplicate, adopt_in_place, adopt_stored, problem};

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
pub fn merge(root: &Path, ix: &mut Index, logger: &Logger, abort: &AtomicBool) -> FlowDone<Result> {
    let mut res = Result::default();
    for sub in find_sub_indexes(root) {
        if abort.load(Ordering::Relaxed) {
            return FlowDone::Aborted;
        }
        merge_one(One {
            root,
            sub: &sub,
            ix: &mut *ix,
            logger,
            abort,
            res: &mut res,
        });
    }
    FlowDone::Done(res)
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
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
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
    dirs.sort_by(|a, b| slashes(b).cmp(&slashes(a)).then_with(|| a.cmp(b)));
    dirs
}

fn slashes(rel: &str) -> usize {
    rel.bytes().filter(|&c| c == b'/').count()
}

/// Per-record adoption context (keeps arity small).
struct Ctx<'a> {
    root: &'a Path,
    sub: &'a str,
    sub_abs: &'a Path,
    by_loc: &'a std::collections::HashMap<String, hash::Outcome>,
    ix: &'a mut Index,
    res: &'a mut Result,
}

/// Single sub-index merge context (keeps arity small).
struct One<'a> {
    root: &'a Path,
    sub: &'a str,
    ix: &'a mut Index,
    logger: &'a Logger,
    abort: &'a AtomicBool,
    res: &'a mut Result,
}

fn merge_one(ctx: One) {
    let sub_abs = ctx.root.join(library::from_slash(ctx.sub));
    let sub_ix = match Index::load(&sub_abs) {
        Ok(ix) => ix,
        Err(e) => {
            ctx.res
                .problems
                .push(format!("{}/.index.json: {e}", ctx.sub));
            ctx.res.skipped.push(ctx.sub.to_string());
            return;
        }
    };
    let by_loc = hash_sub_records(&sub_abs, &sub_ix, ctx.logger, ctx.abort);
    let mut failed = false;
    for rec in &sub_ix.files {
        let mut inner = Ctx {
            root: ctx.root,
            sub: ctx.sub,
            sub_abs: &sub_abs,
            by_loc: &by_loc,
            ix: &mut *ctx.ix,
            res: &mut *ctx.res,
        };
        if !adopt_record(rec, &mut inner) {
            failed = true;
        }
    }
    if failed {
        ctx.res.skipped.push(ctx.sub.to_string());
        return;
    }
    if let Err(e) = std::fs::remove_file(sub_abs.join(index::FILE_NAME)) {
        ctx.res
            .problems
            .push(format!("{}/.index.json: {e}", ctx.sub));
        return;
    }
    ctx.res.indexes += 1;
    let _ = std::fs::remove_dir_all(sub_abs.join(index::STORE_DIR));
    ctx.logger
        .info(format!("adopted nested library {}", ctx.sub));
}

/// Hash every indexed location of a sub-library, keyed by
/// slash-relative path inside it.
fn hash_sub_records(
    sub_abs: &Path,
    sub_ix: &Index,
    logger: &Logger,
    abort: &AtomicBool,
) -> std::collections::HashMap<String, hash::Outcome> {
    let rels: Vec<String> = sub_ix
        .files
        .iter()
        .map(|r| r.location().to_string())
        .collect();
    let abs: Vec<PathBuf> = rels
        .iter()
        .map(|r| sub_abs.join(library::from_slash(r)))
        .collect();
    let out = hash::files(&abs, hash::workers(), abort, |done, total, path| {
        logger.progress("hash", done, total, &slash_rel(sub_abs, path));
    });
    out.into_iter()
        .map(|o| {
            let rel = slash_rel(sub_abs, &o.path);
            (rel, o)
        })
        .collect()
}

fn adopt_record(rec: &Record, ctx: &mut Ctx) -> bool {
    if let Some(dup) = ctx.ix.find(&rec.id).cloned()
        && moved_earlier(ctx.root, rec, &dup)
    {
        return true;
    }
    let sub = ctx.sub;
    let outcome = match ctx.by_loc.get(rec.location()) {
        Some(o) if o.error.is_none() => o.clone(),
        _ => return problem(ctx.res, &format!("{sub}/{}: missing", rec.location())),
    };
    if outcome.hash != rec.id {
        return problem(
            ctx.res,
            &format!("{sub}/{}: content changed", rec.location()),
        );
    }
    if let Some(dup) = ctx.ix.find(&rec.id).cloned() {
        return adopt_duplicate(ctx.sub, ctx.sub_abs, rec, &dup, &mut *ctx.res);
    }
    if rec.stored() {
        return adopt_stored(StoredCtx {
            root: ctx.root,
            sub: ctx.sub,
            sub_abs: ctx.sub_abs,
            rec,
            size: outcome.size,
            ix: &mut *ctx.ix,
            res: &mut *ctx.res,
        });
    }
    let want = format!("{sub}/{}", rec.original_rel);
    if ctx.ix.files.iter().any(|r| r.location() == want) {
        return problem(
            ctx.res,
            &format!(
                "{sub}/{}: path already indexed with different content",
                rec.location()
            ),
        );
    }
    adopt_in_place(ctx.sub, rec, outcome.size, &mut *ctx.ix, &mut *ctx.res);
    true
}

/// Stored-record adoption context (keeps arity small).
pub(crate) struct StoredCtx<'a> {
    root: &'a Path,
    sub: &'a str,
    sub_abs: &'a Path,
    rec: &'a Record,
    size: i64,
    ix: &'a mut Index,
    res: &'a mut Result,
}

/// Whether rec was adopted by a previous pass: a stored record
/// whose content already lives at the parent location.
fn moved_earlier(root: &Path, rec: &Record, dup: &Record) -> bool {
    if !rec.stored() || !dup.stored() {
        return false;
    }
    std::fs::symlink_metadata(root.join(library::from_slash(&dup.stored_rel))).is_ok()
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
        let calm = AtomicBool::new(false);
        let FlowDone::Done(res) = merge(&dir, &mut ix, &logger, &calm) else {
            panic!("aborted");
        };
        assert!(res.problems.is_empty(), "{:?}", res.problems);
        assert_eq!(res.indexes, 1);
        assert_eq!(ix.files.len(), 1);
        assert_eq!(ix.files[0].original_rel, "sub/a.txt");
        assert!(!sub.join(index::FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
