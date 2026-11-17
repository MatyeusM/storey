//! `mv` wiring: lossless renames of display name, file, links.

use crate::{EXIT_OK, emit, fail, usage_err};
use storey_core::log::Logger;

/// `storey mv`: rename display name, file, and view links.
pub fn cmd_mv(args: &[String], as_json: bool, logger: &Logger) -> i32 {
    let (dry, target, new_name) = match parse_mv_args(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    if let Err(e) = storey_core::tags::check_name(&new_name) {
        return usage_err(&format!("mv: {e}"));
    }
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    let pos = match crate::resolve_target(&opened.root, &opened.ix, &target, "mv", as_json) {
        Ok(pos) => pos,
        Err(()) => return crate::EXIT_FAILURE,
    };
    let old = opened.ix.files[pos].display_name.clone();
    let id = opened.ix.files[pos].id.clone();
    if new_name == old {
        return unchanged_name(as_json, id, old);
    }
    let old_links = storey_core::remove::all_view_links(&opened.ix);
    let plan = match storey_core::rename::plan_rename(&opened.root, &opened.ix, pos, &new_name) {
        Ok(plan) => plan,
        Err(msg) => return usage_err(&msg),
    };
    if dry {
        return dry_move(as_json, id, old, plan.new_name, old_links.len());
    }
    let rep = match run_rename(&mut opened, pos, &plan, &old_links, logger) {
        Ok(errors) => storey_core::rename::MvReport {
            id,
            old,
            new: plan.new_name,
            link_errors: errors,
        },
        Err(e) => return fail(&format!("{e}")),
    };
    emit("mv", as_json, &rep);
    if !as_json {
        println!("renamed {} -> {}", rep.old, rep.new);
    }
    EXIT_OK
}

/// Apply the rename under the held lock, returning any link warnings.
fn run_rename(
    opened: &mut crate::Opened,
    pos: usize,
    plan: &storey_core::rename::RenamePlan,
    old_links: &[String],
    logger: &Logger,
) -> anyhow::Result<Vec<String>> {
    storey_core::rename::apply_rename(
        &mut opened.ix,
        storey_core::rename::RenameCtx {
            root: &opened.root,
            pos,
            plan,
            old_links,
            logger,
        },
    )
}

/// Report an unchanged name without touching anything.
fn unchanged_name(as_json: bool, id: String, old: String) -> i32 {
    let rep = storey_core::rename::MvReport {
        new: old.clone(),
        id,
        old,
        link_errors: vec![],
    };
    emit("mv", as_json, &rep);
    if !as_json {
        println!("name unchanged");
    }
    EXIT_OK
}

/// Preview a rename without touching files or the index.
fn dry_move(as_json: bool, id: String, old: String, new_name: String, links: usize) -> i32 {
    let rep = storey_core::rename::MvReport {
        id,
        old,
        new: new_name,
        link_errors: vec![],
    };
    emit("mv", as_json, &rep);
    if !as_json {
        println!(
            "would rename {} -> {} ({} link(s))",
            rep.old, rep.new, links
        );
    }
    EXIT_OK
}

fn parse_mv_args(args: &[String]) -> Result<(bool, String, String), String> {
    let mut dry = false;
    let mut positionals = Vec::new();
    for a in args {
        if a == "--dry-run" {
            dry = true;
            continue;
        }
        positionals.push(a.clone());
    }
    if positionals.len() != 2 {
        return Err("mv takes <target> <new-name>".into());
    }
    Ok((dry, positionals.remove(0), positionals.remove(0)))
}
