//! Dry-run previews for `init`/`update`: simulate on a throwaway index,
//! never adopt, write, or delete anything.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use anyhow::Result;

use crate::flows::{InitReport, UpdateReport};
use crate::index::Index;
use crate::intake;
use crate::log::Logger;

/// Preview `init` on a throwaway copy: no adoption (it moves files),
/// no index write, no deletes.
pub fn dry_init(root: &Path, ix: &Index, logger: &Logger) -> Result<InitReport> {
    let mut rep = InitReport {
        added: vec![],
        duplicates: vec![],
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
    rep.duplicates.extend(scan.duplicates);
    rep.moved.extend(scan.moved);
    Ok(rep)
}

/// Preview `update` on a throwaway copy. Composes with `--verify`.
pub fn dry_update(root: &Path, ix: &Index, verify: bool, logger: &Logger) -> Result<UpdateReport> {
    let mut rep = UpdateReport {
        added: vec![],
        duplicates: vec![],
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
    rep.duplicates.extend(scan.duplicates);
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
