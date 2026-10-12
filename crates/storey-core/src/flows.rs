//! Init/update orchestration over intake mechanics.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use serde::Serialize;

use crate::adopt;
use crate::index::Index;
use crate::intake;
use crate::log::Logger;

/// Outcome of an intake flow: report payload plus whether problems
/// (report-only) or an abort occurred.
pub enum FlowDone<T> {
    /// Finished; problems (if any) ride in the report.
    Done(T),
    /// Abort flag set: index already saved by the caller path.
    Aborted,
}

// --- init / update flows -------------------------------------------------

/// `init` payload: intake plus adoption.
#[derive(Debug, Serialize)]
pub struct InitReport {
    pub added: Vec<String>,
    pub purged: Vec<String>,
    pub moved: Vec<String>,
    pub adopted: Vec<String>,
    pub problems: Vec<String>,
}

/// `update` payload: intake plus adoption, removal, and verification.
#[derive(Debug, Serialize)]
pub struct UpdateReport {
    pub added: Vec<String>,
    pub purged: Vec<String>,
    pub moved: Vec<String>,
    pub adopted: Vec<String>,
    pub removed: Vec<String>,
    pub reverified: usize,
    pub problems: Vec<String>,
}

/// Adopt nested libraries, recording the outcome. Returns the
/// shielded sub-index list for the file scan.
fn adopt_into(
    ix: &mut Index,
    root: &Path,
    logger: &Logger,
    adopted: &mut Vec<String>,
    problems: &mut Vec<String>,
    purged: &mut Vec<String>,
) -> Vec<String> {
    let merged = adopt::merge(root, ix, logger);
    adopted.extend(merged.adopted);
    problems.extend(merged.problems);
    purged.extend(merged.purged);
    merged.skipped
}

fn aborted(abort: &AtomicBool) -> bool {
    abort.load(Ordering::Relaxed)
}

/// Execute `init` on an open index: adopt, hash, fill, save.
/// A partial index beats re-adopting from scratch (re-run `update`
/// to continue after an abort).
pub fn exec_init(
    root: &Path,
    ix: &mut Index,
    logger: &Logger,
    abort: &AtomicBool,
) -> Result<FlowDone<InitReport>> {
    let mut rep = InitReport {
        added: vec![],
        purged: vec![],
        moved: vec![],
        adopted: vec![],
        problems: vec![],
    };
    let skipped = adopt_into(
        ix,
        root,
        logger,
        &mut rep.adopted,
        &mut rep.problems,
        &mut rep.purged,
    );
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let rels = intake::candidates(root, ix, &skipped)?;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let hashed = intake::hash_rels(root, &rels, logger, "hash");
    rep.problems.extend(hashed.problems);
    match intake::fill_index(ix, root, &hashed.ordered, &hashed.by_rel, false, abort) {
        Ok(scan) => {
            rep.added.extend(scan.added);
            rep.purged.extend(scan.purged);
            rep.moved.extend(scan.moved);
        }
        Err(_) if aborted(abort) => return Ok(FlowDone::Aborted),
        Err(e) => return Err(e),
    }
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    ix.save(root)?;
    Ok(FlowDone::Done(rep))
}

/// Preview `init` on a throwaway copy: no adoption (it moves
/// files), no index write, no deletes.
pub fn dry_init(root: &Path, ix: &Index, logger: &Logger) -> Result<InitReport> {
    let mut rep = InitReport {
        added: vec![],
        purged: vec![],
        moved: vec![],
        adopted: vec![],
        problems: vec![],
    };
    let rels = intake::candidates(root, ix, &[])?;
    let hashed = intake::hash_rels(root, &rels, logger, "hash");
    rep.problems.extend(hashed.problems);
    let mut sim = intake::copy_index(ix);
    let abort = AtomicBool::new(false);
    let scan = intake::fill_index(
        &mut sim,
        root,
        &hashed.ordered,
        &hashed.by_rel,
        true,
        &abort,
    )?;
    rep.added.extend(scan.added);
    rep.purged.extend(scan.purged);
    rep.moved.extend(scan.moved);
    Ok(rep)
}

/// Execute trusted `update` on an open index: adopt, hash intake,
/// optionally verify, save.
pub fn exec_update(
    root: &Path,
    ix: &mut Index,
    verify: bool,
    logger: &Logger,
    abort: &AtomicBool,
) -> Result<FlowDone<UpdateReport>> {
    let mut rep = UpdateReport {
        added: vec![],
        purged: vec![],
        moved: vec![],
        adopted: vec![],
        removed: vec![],
        reverified: 0,
        problems: vec![],
    };
    let skipped = adopt_into(
        ix,
        root,
        logger,
        &mut rep.adopted,
        &mut rep.problems,
        &mut rep.purged,
    );
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let rels = intake::candidates(root, ix, &skipped)?;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let hashed = intake::hash_rels(root, &rels, logger, "hash");
    rep.problems.extend(hashed.problems);
    match intake::fill_index(ix, root, &hashed.ordered, &hashed.by_rel, false, abort) {
        Ok(scan) => {
            rep.added.extend(scan.added);
            rep.purged = [rep.purged, scan.purged].concat();
            rep.moved.extend(scan.moved);
        }
        Err(_) if aborted(abort) => return Ok(FlowDone::Aborted),
        Err(e) => return Err(e),
    }
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    if verify {
        rep.problems.extend(intake::reconcile_store(root, ix));
        let (more, reverified, removed) = intake::reverify(root, ix, logger, abort);
        rep.problems.extend(more);
        rep.reverified = reverified;
        rep.removed = removed;
    }
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    ix.save(root)?;
    Ok(FlowDone::Done(rep))
}

/// Preview `update` on a throwaway copy. Composes with `--verify`.
pub fn dry_update(root: &Path, ix: &Index, verify: bool, logger: &Logger) -> Result<UpdateReport> {
    let mut rep = UpdateReport {
        added: vec![],
        purged: vec![],
        moved: vec![],
        adopted: vec![],
        removed: vec![],
        reverified: 0,
        problems: vec![],
    };
    let rels = intake::candidates(root, ix, &[])?;
    let hashed = intake::hash_rels(root, &rels, logger, "hash");
    rep.problems.extend(hashed.problems);
    let mut sim = intake::copy_index(ix);
    let abort = AtomicBool::new(false);
    let scan = intake::fill_index(
        &mut sim,
        root,
        &hashed.ordered,
        &hashed.by_rel,
        true,
        &abort,
    )?;
    rep.added.extend(scan.added);
    rep.purged.extend(scan.purged);
    rep.moved.extend(scan.moved);
    if verify {
        rep.problems.extend(intake::reconcile_store(root, &mut sim));
        let (more, reverified, removed) = intake::reverify(root, &mut sim, logger, &abort);
        rep.problems.extend(more);
        rep.reverified = reverified;
        rep.removed = removed;
    }
    Ok(rep)
}

/// Abort-aware index save for the emergency path.
pub fn save_index(root: &Path, ix: &Index) -> Result<()> {
    ix.save(root)
}
