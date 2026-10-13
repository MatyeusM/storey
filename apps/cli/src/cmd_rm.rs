//! `rm`, `mv`, `restore` wiring: permanent deletes, lossless
//! renames, and store-to-original restoration.
//!
//! Argument parsing mirrors the reference CLI; the flows live in
//! `storey-core::{remove, rename, restore}`.

use crate::{EXIT_OK, emit, fail, usage_err};
use storey_core::log::Logger;

/// `storey remove|rm`: delete links, file, and record. Permanent.
pub fn cmd_rm(args: &[String], as_json: bool, _logger: &Logger) -> i32 {
    let (dry, target) = match parse_rm_args(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    let pos = match crate::resolve_target(&opened.root, &opened.ix, &target, "rm", as_json) {
        Ok(pos) => pos,
        Err(()) => return crate::EXIT_FAILURE,
    };
    let rec = opened.ix.files[pos].clone();
    let links = storey_core::remove::links_for(&opened.ix, &rec.id);
    let rep = storey_core::remove::RmReport {
        removed: rec.id.clone(),
    };
    if dry {
        emit("rm", as_json, &rep);
        if !as_json {
            println!(
                "would remove {} ({}) and {} link(s)",
                rec.id,
                rec.location(),
                links.len()
            );
        }
        return EXIT_OK;
    }
    if let Err(e) = storey_core::remove::exec_rm(&opened.root, &mut opened.ix, pos) {
        return fail(&format!("{e}"));
    }
    if let Err(e) = opened.ix.save(&opened.root) {
        return fail(&format!("{e}"));
    }
    emit("rm", as_json, &rep);
    if !as_json {
        println!("removed {} ({} link(s))", rep.removed, links.len());
    }
    EXIT_OK
}

fn parse_rm_args(args: &[String]) -> Result<(bool, String), String> {
    let mut dry = false;
    let mut target = String::new();
    for a in args {
        if a == "--dry-run" {
            dry = true;
            continue;
        }
        if !target.is_empty() {
            return Err("rm takes exactly one <target>".into());
        }
        target = a.clone();
    }
    if target.is_empty() {
        return Err("rm takes exactly one <target>".into());
    }
    Ok((dry, target))
}
