//! `mv` wiring: lossless renames of display name, file, links.

use crate::{EXIT_OK, emit, fail, usage_err};
use storey_core::log::Logger;

/// `storey mv`: rename display name, file, and view links.
pub fn cmd_mv(args: &[String], as_json: bool, _logger: &Logger) -> i32 {
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
    let old_links = storey_core::remove::links_for(&opened.ix, &id);
    let plan = match storey_core::rename::plan_rename(&opened.root, &opened.ix, pos, &new_name) {
        Ok(plan) => plan,
        Err(msg) => return usage_err(&msg),
    };
    if dry {
        return dry_move(as_json, id, old, plan.new_name, old_links.len());
    }
    if let Err(e) =
        storey_core::rename::apply_rename(&opened.root, &mut opened.ix, pos, &plan, &old_links)
    {
        return fail(&format!("{e}"));
    }
    if let Err(e) = opened.ix.save(&opened.root) {
        return fail(&format!("{e}"));
    }
    let rep = storey_core::rename::MvReport {
        id,
        old,
        new: plan.new_name,
    };
    emit("mv", as_json, &rep);
    if !as_json {
        println!("renamed {} -> {}", rep.old, rep.new);
    }
    EXIT_OK
}

/// Report an unchanged name without touching anything.
fn unchanged_name(as_json: bool, id: String, old: String) -> i32 {
    let rep = storey_core::rename::MvReport {
        new: old.clone(),
        id,
        old,
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
