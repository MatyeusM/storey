//! `init` and `update` wiring: locks, abort trapping, reports.
//!
//! Flows run in `storey-core::flows`; this module owns process
//! concerns (cwd, locks, signals, emergency saves) and printing.

use crate::{EXIT_FAILURE, EXIT_OK, emit, fail};
use storey_core::flows::{self, FlowDone};
use storey_core::index::Index;
use storey_core::log::Logger;

/// Run `init`: refuse inside libraries, lock, adopt, hash, save.
pub fn cmd_init(as_json: bool, dry: bool, logger: &Logger) -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("{e}")),
    };
    if storey_core::library::find_root(&cwd).is_ok() {
        return fail("already inside a library");
    }
    let _lock = match storey_core::lock::acquire(&cwd) {
        Ok(guard) => guard,
        Err(msg) => return fail(&msg),
    };
    if dry {
        return dry_init_report(&cwd, as_json, logger);
    }
    let abort = crate::trap_abort();
    let mut ix = Index::new();
    match flows::exec_init(&cwd, &mut ix, logger, &abort) {
        Ok(FlowDone::Done(rep)) => {
            emit("init", as_json, &rep);
            if !as_json {
                println!(
                    "initialized library at {}\nadded: {}\nadopted: {}\npurged (duplicates): {}\nmoved: {}\nproblems: {}",
                    cwd.display(),
                    rep.added.len(),
                    rep.adopted.len(),
                    rep.purged.len(),
                    rep.moved.len(),
                    rep.problems.len()
                );
                for p in &rep.problems {
                    println!("  {p}");
                }
            }
            if rep.problems.is_empty() {
                EXIT_OK
            } else {
                EXIT_FAILURE
            }
        }
        Ok(FlowDone::Aborted) => crate::emergency_save(&cwd, &ix),
        Err(e) => fail(&format!("{e}")),
    }
}

/// Run trusted `update` (plus `--verify`): adopt, hash, save.
pub fn cmd_update(as_json: bool, verify: bool, dry: bool, logger: &Logger) -> i32 {
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    let root = opened.root.clone();
    if dry {
        return dry_update_report(&root, &opened.ix, verify, as_json, logger);
    }
    let abort = crate::trap_abort();
    match flows::exec_update(&root, &mut opened.ix, verify, logger, &abort) {
        Ok(FlowDone::Done(rep)) => report_update(as_json, &rep),
        Ok(FlowDone::Aborted) => crate::emergency_save(&root, &opened.ix),
        Err(e) => fail(&format!("{e}")),
    }
}

fn dry_init_report(root: &std::path::Path, as_json: bool, logger: &Logger) -> i32 {
    let ix = Index::new();
    match flows::dry_init(root, &ix, logger) {
        Ok(rep) => {
            emit("init", as_json, &rep);
            if !as_json {
                println!(
                    "init (dry run, adoption skipped)\nwould add: {}\nwould purge (duplicates): {}\nwould repoint (moved): {}\nproblems: {}",
                    rep.added.len(),
                    rep.purged.len(),
                    rep.moved.len(),
                    rep.problems.len()
                );
            }
            EXIT_OK
        }
        Err(e) => fail(&format!("{e}")),
    }
}

fn dry_update_report(
    root: &std::path::Path,
    ix: &Index,
    verify: bool,
    as_json: bool,
    logger: &Logger,
) -> i32 {
    match flows::dry_update(root, ix, verify, logger) {
        Ok(rep) => {
            emit("update", as_json, &rep);
            if !as_json {
                println!(
                    "update (dry run, adoption skipped)\nwould add: {}\nwould purge (duplicates): {}\nwould repoint (moved): {}\nremoved (missing): {}\nreverified: {}\nproblems: {}",
                    rep.added.len(),
                    rep.purged.len(),
                    rep.moved.len(),
                    rep.removed.len(),
                    rep.reverified,
                    rep.problems.len()
                );
            }
            EXIT_OK
        }
        Err(e) => fail(&format!("{e}")),
    }
}

fn report_update(as_json: bool, rep: &flows::UpdateReport) -> i32 {
    emit("update", as_json, rep);
    if !as_json {
        println!(
            "added: {}\nadopted: {}\npurged (duplicates): {}\nmoved: {}\nremoved (missing): {}\nreverified: {}\nproblems: {}",
            rep.added.len(),
            rep.adopted.len(),
            rep.purged.len(),
            rep.moved.len(),
            rep.removed.len(),
            rep.reverified,
            rep.problems.len()
        );
        for p in &rep.problems {
            println!("  {p}");
        }
    }
    if rep.problems.is_empty() {
        EXIT_OK
    } else {
        EXIT_FAILURE
    }
}
