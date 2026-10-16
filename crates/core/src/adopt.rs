//! Nested-library adoption: merge sub-indexes record by record
//! into the parent with tags and names intact, and remove adopted
//! `.index.json` files. Resolution is existence-based (no hashing):
//! a record is resolved when its file exists at the recorded
//! location. Duplicates are purged from disk. Only moves and
//! deletes here — never copies.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::FlowDone;
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
    /// Unresolved entries, left in place.
    pub problems: Vec<String>,
    /// Sub-index dirs with problems (files stay untouched).
    pub skipped: Vec<String>,
}

/// Fold every nested library under root into `ix` (root itself is
/// excluded, as is anything inside `.files/`). A sub-index is
/// deleted only when all of its records adopted cleanly — unless
/// `force` is set, which deletes it anyway: resolved records are
/// adopted, unresolved ones are dropped (not added, not reported as
/// problems) and their files are left for a fresh scan.
pub fn merge(
    root: &Path,
    ix: &mut Index,
    logger: &Logger,
    abort: &AtomicBool,
    force: bool,
) -> FlowDone<Result> {
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
            res: &mut res,
            force,
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
    ix: &'a mut Index,
    res: &'a mut Result,
}

/// Single sub-index merge context (keeps arity small).
struct One<'a> {
    root: &'a Path,
    sub: &'a str,
    ix: &'a mut Index,
    logger: &'a Logger,
    res: &'a mut Result,
    force: bool,
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
    let base = ctx.res.problems.len();
    let mut failed = false;
    for rec in &sub_ix.files {
        let mut inner = Ctx {
            root: ctx.root,
            sub: ctx.sub,
            sub_abs: &sub_abs,
            ix: &mut *ctx.ix,
            res: &mut *ctx.res,
        };
        if !adopt_record(rec, &mut inner) {
            failed = true;
        }
    }
    if failed && !ctx.force {
        ctx.logger.warn(format!(
            "keeping nested library {} (unresolved entries)",
            ctx.sub
        ));
        ctx.res.skipped.push(ctx.sub.to_string());
        return;
    }
    let mut dropped = 0;
    if failed {
        dropped = ctx.res.problems.len() - base;
        ctx.res.problems.truncate(base);
    }
    finish_one(ctx, &sub_abs, dropped);
}

/// Remove the sub-index after its records resolved (`dropped` counts
/// records discarded under `force`): move leftover store files up,
/// delete the index, and log the outcome.
fn finish_one(ctx: One, sub_abs: &Path, dropped: usize) {
    if !mv::move_store_leftovers(ctx.root, sub_abs, ctx.ix, &mut *ctx.res) {
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
    let _ = std::fs::remove_dir(sub_abs.join(index::STORE_DIR));
    if dropped > 0 {
        ctx.logger.info(format!(
            "adopted nested library {} (--force: {dropped} unresolved entries dropped, this is not a problem)",
            ctx.sub
        ));
    } else {
        ctx.logger
            .info(format!("adopted nested library {}", ctx.sub));
    }
}

fn adopt_record(rec: &Record, ctx: &mut Ctx) -> bool {
    if let Some(dup) = ctx.ix.find(&rec.id).cloned()
        && moved_earlier(ctx.root, ctx.sub_abs, rec, &dup)
    {
        return true;
    }
    let sub = ctx.sub;
    // Resolved = the file exists at its recorded location. No
    // hashing: sizes and ids are taken from the child record on
    // trust (`update --verify` is the content check).
    if std::fs::symlink_metadata(ctx.sub_abs.join(library::from_slash(rec.location()))).is_err() {
        return problem(ctx.res, &format!("{sub}/{}: missing", rec.location()));
    }
    if let Some(dup) = ctx.ix.find(&rec.id).cloned() {
        return adopt_duplicate(mv::DupCtx {
            root: ctx.root,
            sub: ctx.sub,
            sub_abs: ctx.sub_abs,
            rec,
            dup: &dup,
            res: &mut *ctx.res,
        });
    }
    if rec.stored() {
        return adopt_stored(StoredCtx {
            root: ctx.root,
            sub: ctx.sub,
            sub_abs: ctx.sub_abs,
            rec,
            size: rec.size,
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
    adopt_in_place(ctx.sub, rec, rec.size, &mut *ctx.ix, &mut *ctx.res);
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

/// Whether rec was adopted by an earlier record in this run: a
/// stored record whose child copy is already gone while the parent
/// copy is on disk. The shortcut must require the child to be gone;
/// otherwise the untouched child blob would be re-imported as an
/// orphan by `move_store_leftovers`.
fn moved_earlier(root: &Path, sub_abs: &Path, rec: &Record, dup: &Record) -> bool {
    if !rec.stored() || !dup.stored() {
        return false;
    }
    let child_gone =
        std::fs::symlink_metadata(sub_abs.join(library::from_slash(rec.location()))).is_err();
    let parent_ok =
        std::fs::symlink_metadata(root.join(library::from_slash(&dup.stored_rel))).is_ok();
    child_gone && parent_ok
}

#[cfg(test)]
#[path = "adopt_tests.rs"]
mod tests;
