//! Init/update orchestration over intake mechanics.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use serde::Serialize;

pub use crate::FlowDone;
use crate::adopt;
use crate::index::Index;
use crate::intake;
use crate::log::Logger;

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

impl UpdateReport {
    /// One-line summary for notices (`3 added · 1 removed · 2 problems`).
    pub fn summary(&self) -> String {
        let mut parts = vec![
            format!("{} added", self.added.len()),
            format!("{} removed", self.removed.len()),
        ];
        if self.reverified > 0 {
            parts.push(format!("{} re-hashed", self.reverified));
        }
        if !self.problems.is_empty() {
            parts.push(format!("{} problems", self.problems.len()));
        }
        parts.join(" · ")
    }
}

/// Adopted nested libraries: outcome plus shield list.
struct Adopted {
    skipped: Vec<String>,
    adopted: Vec<String>,
    problems: Vec<String>,
    purged: Vec<String>,
}

/// Adopt nested libraries, recording the outcome.
fn adopt_into(
    ix: &mut Index,
    root: &Path,
    logger: &Logger,
    abort: &AtomicBool,
    force: bool,
) -> FlowDone<Adopted> {
    match adopt::merge(root, ix, logger, abort, force) {
        FlowDone::Done(merged) => FlowDone::Done(Adopted {
            skipped: merged.skipped,
            adopted: merged.adopted,
            problems: merged.problems,
            purged: merged.purged,
        }),
        FlowDone::Aborted => FlowDone::Aborted,
    }
}

/// Salvage partial hashing into the live index without deletes,
/// so an abort keeps whatever finished.
fn salvage(ix: &mut Index, root: &Path, hashed: &intake::Hashed) {
    let calm = AtomicBool::new(false);
    let _ = intake::fill_index(ix, root, hashed, true, &calm);
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
    force: bool,
) -> Result<FlowDone<InitReport>> {
    let mut rep = InitReport {
        added: vec![],
        purged: vec![],
        moved: vec![],
        adopted: vec![],
        problems: vec![],
    };
    let got = match adopt_into(ix, root, logger, abort, force) {
        FlowDone::Done(got) => got,
        FlowDone::Aborted => return Ok(FlowDone::Aborted),
    };
    rep.adopted.extend(got.adopted);
    rep.problems.extend(got.problems);
    rep.purged.extend(got.purged);
    let skipped = got.skipped;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let rels = intake::candidates(root, ix, &skipped)?;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let mut hashed = intake::hash_rels(root, &rels, logger, "hash", abort);
    if aborted(abort) {
        salvage(ix, root, &hashed);
        return Ok(FlowDone::Aborted);
    }
    rep.problems.extend(std::mem::take(&mut hashed.problems));
    match intake::fill_index(ix, root, &hashed, false, abort) {
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
    let calm = AtomicBool::new(false);
    let mut hashed = intake::hash_rels(root, &rels, logger, "hash", &calm);
    rep.problems.extend(std::mem::take(&mut hashed.problems));
    let mut sim = intake::copy_index(ix);
    let scan = intake::fill_index(&mut sim, root, &hashed, true, &calm)?;
    rep.added.extend(scan.added);
    rep.purged.extend(scan.purged);
    rep.moved.extend(scan.moved);
    Ok(rep)
}

/// Execute trusted `update` on an open index: adopt, hash intake,
/// optionally verify, save.
#[allow(clippy::too_many_arguments)]
pub fn exec_update(
    root: &Path,
    ix: &mut Index,
    verify: bool,
    logger: &Logger,
    abort: &AtomicBool,
    force: bool,
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
    let got = match adopt_into(ix, root, logger, abort, force) {
        FlowDone::Done(got) => got,
        FlowDone::Aborted => return Ok(FlowDone::Aborted),
    };
    rep.adopted.extend(got.adopted);
    rep.problems.extend(got.problems);
    rep.purged.extend(got.purged);
    let skipped = got.skipped;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let rels = intake::candidates(root, ix, &skipped)?;
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    let mut hashed = intake::hash_rels(root, &rels, logger, "hash", abort);
    if aborted(abort) {
        salvage(ix, root, &hashed);
        return Ok(FlowDone::Aborted);
    }
    rep.problems.extend(std::mem::take(&mut hashed.problems));
    match intake::fill_index(ix, root, &hashed, false, abort) {
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
        verify_index(root, ix, &mut rep, logger, abort);
    }
    if aborted(abort) {
        return Ok(FlowDone::Aborted);
    }
    ix.save(root)?;
    Ok(FlowDone::Done(rep))
}

/// Reconcile plus reverify for `--verify`, folding into the report.
fn verify_index(
    root: &Path,
    ix: &mut Index,
    rep: &mut UpdateReport,
    logger: &Logger,
    abort: &AtomicBool,
) {
    rep.problems.extend(intake::reconcile_store(root, ix));
    let (more, reverified, removed) = intake::reverify(root, ix, logger, abort);
    rep.problems.extend(more);
    rep.reverified = reverified;
    rep.removed = removed;
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
    let calm = AtomicBool::new(false);
    let mut hashed = intake::hash_rels(root, &rels, logger, "hash", &calm);
    rep.problems.extend(std::mem::take(&mut hashed.problems));
    let mut sim = intake::copy_index(ix);
    let scan = intake::fill_index(&mut sim, root, &hashed, true, &calm)?;
    rep.added.extend(scan.added);
    rep.purged.extend(scan.purged);
    rep.moved.extend(scan.moved);
    if verify {
        rep.problems.extend(intake::reconcile_store(root, &mut sim));
        let (more, reverified, removed) = intake::reverify(root, &mut sim, logger, &calm);
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
