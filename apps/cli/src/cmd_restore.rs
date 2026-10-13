//! `restore` wiring: clear view artifacts, move objects back.

use storey_core::flows::FlowDone;
use storey_core::index::Index;
use storey_core::log::Logger;

use crate::{EXIT_OK, emit, fail, usage_err};

/// `storey restore`: remove view artifacts, move objects back.
pub fn cmd_restore(args: &[String], as_json: bool, _logger: &Logger) -> i32 {
    let (force, dry) = match parse_restore_args(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    // Heal crash residue before planning (like `view`); dry previews
    // and skipped counts then see the same index `exec` will use.
    storey_core::intake::reconcile_store(&opened.root, &mut opened.ix);
    let stored = storey_core::restore::stored_positions(&opened.ix);
    let rep = storey_core::restore::RestoreReport {
        restored: 0,
        skipped: opened.ix.files.len() - stored.len(),
        conflicts: Some(vec![]),
        problems: vec![],
        untracked: vec![],
        moves: Default::default(),
    };
    if stored.is_empty() {
        emit("restore", as_json, &rep);
        if !as_json {
            println!("nothing to restore");
        }
        return EXIT_OK;
    }
    let rep = match guard_restore(&mut opened, &stored, force, as_json, rep) {
        Ok(rep) => rep,
        Err(code) => return code,
    };
    run_restore(&mut opened, &stored, dry, as_json, rep)
}

/// Execute the dry preview or the live restore.
fn run_restore(
    opened: &mut crate::Opened,
    stored: &[usize],
    dry: bool,
    as_json: bool,
    mut rep: storey_core::restore::RestoreReport,
) -> i32 {
    if dry {
        return dry_restore(as_json, &opened.ix, stored, rep);
    }
    let abort = crate::trap_abort();
    match storey_core::restore::exec_restore(&opened.root, &mut opened.ix, &abort) {
        Ok(FlowDone::Done(restored)) => {
            rep.restored = restored;
            emit("restore", as_json, &rep);
            if !as_json {
                println!(
                    "restored: {}\nskipped (in place): {}",
                    rep.restored, rep.skipped
                );
            }
            EXIT_OK
        }
        Ok(FlowDone::Aborted) => crate::emergency_save(&opened.root, &opened.ix),
        Err(e) => fail(&format!("{e}")),
    }
}

fn parse_restore_args(args: &[String]) -> Result<(bool, bool), String> {
    let mut force = false;
    let mut dry = false;
    for a in args {
        match a.as_str() {
            "--force" => force = true,
            "--dry-run" => dry = true,
            _ => return Err(format!("restore: unknown argument {a:?}")),
        }
    }
    Ok((force, dry))
}

/// Enforce the untracked/conflict/missing guards, returning the
/// filled report. Guard exits print their own notice.
fn guard_restore(
    opened: &mut crate::Opened,
    stored: &[usize],
    force: bool,
    as_json: bool,
    mut rep: storey_core::restore::RestoreReport,
) -> Result<storey_core::restore::RestoreReport, i32> {
    let untracked = match find_untracked_for(&opened.root, &opened.ix) {
        Ok(u) => u,
        Err(e) => return Err(fail(&format!("{e}"))),
    };
    rep.untracked = untracked;
    let found = storey_core::restore::find_conflicts(&opened.root, &opened.ix, stored);
    rep.conflicts = if found.is_empty() { None } else { Some(found) };
    let has_conflicts = rep
        .conflicts
        .as_ref()
        .map(|c| !c.is_empty())
        .unwrap_or(false);
    if (!rep.untracked.is_empty() || has_conflicts) && !force {
        emit("restore", as_json, &rep);
        report_blocked(&rep.untracked, rep.conflicts.as_deref().unwrap_or(&[]));
        return Err(crate::EXIT_FAILURE);
    }
    let missing = storey_core::restore::missing_stored(&opened.root, &opened.ix, stored);
    if !missing.is_empty() {
        rep.problems = missing.clone();
        emit("restore", as_json, &rep);
        return Err(fail(&format!(
            "restore: {} stored file(s) missing: {}",
            missing.len(),
            missing.join(", ")
        )));
    }
    Ok(rep)
}

/// Preview restoration without touching files or the index.
fn dry_restore(
    as_json: bool,
    ix: &Index,
    stored: &[usize],
    mut rep: storey_core::restore::RestoreReport,
) -> i32 {
    rep.moves = storey_core::restore::restore_moves(ix, stored);
    emit("restore", as_json, &rep);
    if !as_json {
        println!(
            "restore (dry run)\nmove back: {}\nconflicts: {}",
            rep.moves.len(),
            rep.conflicts.as_ref().map(|c| c.len()).unwrap_or(0)
        );
    }
    EXIT_OK
}

/// Untracked files for the restore block check.
fn find_untracked_for(root: &std::path::Path, ix: &Index) -> anyhow::Result<Vec<String>> {
    let tracked = storey_core::library::current_locations(&ix.files);
    let artifacts: std::collections::HashSet<String> =
        storey_core::views::plan_set(&ix.files, &ix.view.by)
            .into_iter()
            .collect();
    storey_core::library::find_untracked(root, &tracked, &artifacts)
}

/// Block notice shared with `view` (restore names its own command).
fn report_blocked(untracked: &[String], conflicts: &[String]) {
    eprintln!("storey: restore: blocked (use --force to destroy/overwrite):");
    for u in untracked {
        eprintln!("  untracked: {u}");
    }
    for c in conflicts {
        eprintln!("  conflict: {c}");
    }
}
