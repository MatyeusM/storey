//! `view` wiring: ingest, wipe, link, record.
//!
//! Argument parsing mirrors the reference CLI; the flow lives in
//! `storey-storey_core::materialize`.

use crate::cmd_view_report::{blocked, dry_view, orphan_lines, print_summary};
use crate::{EXIT_OK, emit, fail, usage_err};
use std::collections::BTreeMap;
use storey_core::flows::FlowDone;
use storey_core::log::Logger;
use storey_core::materialize::{self, ViewReport};

/// Parse `--tags=a,b` (or `--tags a b`... `--tags v`), `--force`,
/// `--dry-run`. `--tags` is required.
fn parse_view_args(args: &[String]) -> Result<(Vec<String>, bool, bool), String> {
    let mut tags_arg: Option<String> = None;
    let mut force = false;
    let mut dry = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--force" {
            force = true;
        } else if a == "--dry-run" {
            dry = true;
        } else if a == "--tags" && i + 1 < args.len() {
            i += 1;
            tags_arg = Some(args[i].clone());
        } else if let Some(v) = a.strip_prefix("--tags=") {
            tags_arg = Some(v.to_string());
        } else {
            return Err(format!("view: unknown argument {a:?}"));
        }
        i += 1;
    }
    let tags_arg = match tags_arg {
        Some(v) => v,
        None => return Err("view: --tags=<a,b,...> is required".into()),
    };
    let by: Vec<String> = tags_arg
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
        .collect();
    Ok((by, force, dry))
}

/// Planned view: report shell plus move/link plans.
struct Planned {
    rep: ViewReport,
    moves: BTreeMap<String, Option<String>>,
    links: Vec<storey_core::views::Link>,
}

/// Scan blockers, enforce guards, and compute the move/link plans.
/// Block exits print their own notice.
fn plan_stage(
    opened: &mut crate::Opened,
    by: &[String],
    force: bool,
    as_json: bool,
) -> Result<Planned, i32> {
    let (untracked, orphans) = match materialize::scan_blockers(&opened.root, &mut opened.ix) {
        Ok(v) => v,
        Err(e) => return Err(fail(&format!("{e}"))),
    };
    let mut rep = ViewReport {
        by: by.to_vec(),
        moved: 0,
        purged: vec![],
        untracked,
        orphans,
        links: 0,
        unsorted: 0,
        link_errors: vec![],
        plan: vec![],
        moves: BTreeMap::new(),
    };
    if !rep.untracked.is_empty() && !force {
        return Err(blocked(as_json, &rep, &[]));
    }
    if !rep.orphans.is_empty() && !force {
        return Err(blocked(as_json, &rep, &orphan_lines(&rep.orphans)));
    }
    let missing = materialize::missing_locations(&opened.root, &opened.ix);
    if !missing.is_empty() {
        return Err(fail(&format!(
            "view: {} indexed file(s) missing: {}",
            missing.len(),
            missing.join(", ")
        )));
    }
    let moves = materialize::compute_moves(&opened.ix);
    let links = materialize::planned_links(&opened.ix, &moves, by);
    rep.links = links.len();
    rep.unsorted = materialize::count_unsorted(&links);
    Ok(Planned { rep, moves, links })
}

/// `storey view`: ingest into `.files/`, wipe, rebuild links.
pub fn cmd_view(args: &[String], as_json: bool, logger: &Logger) -> i32 {
    let (by, force, dry) = match parse_view_args(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    let planned = match plan_stage(&mut opened, &by, force, as_json) {
        Ok(p) => p,
        Err(code) => return code,
    };
    if dry {
        return dry_view(as_json, &by, &planned.moves, &planned.links, planned.rep);
    }
    let abort = crate::trap_abort();
    let ctx = materialize::ExecCtx {
        root: &opened.root,
        by: &by,
        untracked: planned.rep.untracked,
        orphans: planned.rep.orphans,
        moves: &planned.moves,
        links: &planned.links,
        logger,
        abort: &abort,
    };
    match materialize::exec_view(&mut opened.ix, ctx) {
        Ok(FlowDone::Done(done)) => {
            emit("view", as_json, &done);
            print_summary(as_json, &by, done.moved, &done, false);
            EXIT_OK
        }
        Ok(FlowDone::Aborted) => crate::emergency_save(&opened.root, &opened.ix),
        Err(e) => fail(&format!("{e}")),
    }
}
