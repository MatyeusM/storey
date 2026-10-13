//! `status` and `list` wiring: read-only overviews.
//!
//! No locks, no mutation; polling never blocks on writers.

use crate::{EXIT_OK, EXIT_USAGE, emit, fail, usage_err};
use storey_core::ops;

/// `storey status`: read-only overview for humans and UIs.
pub fn cmd_status(args: &[String], as_json: bool, _logger: &storey_core::log::Logger) -> i32 {
    if !args.is_empty() {
        return usage_err("status takes no arguments");
    }
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("{e}")),
    };
    let root = match storey_core::library::find_root(&cwd) {
        Ok(root) => root,
        Err(_) => {
            eprintln!("fatal: not a storey repository");
            return EXIT_USAGE;
        }
    };
    match ops::status_at(&root) {
        Ok(rep) => {
            emit("status", as_json, &rep);
            if !as_json {
                let view = if rep.view.is_empty() {
                    "(none)".to_string()
                } else {
                    rep.view.join(",")
                };
                println!(
                    "storey repository at {}\nindexed: {}\nstored: {}\nuntracked: {}\nview by: {}",
                    rep.root, rep.indexed, rep.stored, rep.untracked, view
                );
            }
            EXIT_OK
        }
        Err(e) => fail(&format!("{e}")),
    }
}

/// `storey list`: records with an optional single filter.
pub fn cmd_list(args: &[String], as_json: bool) -> i32 {
    let filter = match parse_list_args(args) {
        Ok(f) => f,
        Err(msg) => return usage_err(&msg),
    };
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("{e}")),
    };
    let root = match storey_core::library::find_root(&cwd) {
        Ok(root) => root,
        Err(e) => return usage_err(&format!("{e}")),
    };
    let ix = match storey_core::index::Index::load(&root) {
        Ok(ix) => ix,
        Err(e) => return fail(&format!("{e}")),
    };
    let matched = ops::list_records(&ix.files, &filter);
    emit("list", as_json, &matched);
    if !as_json {
        for rec in &matched {
            println!(
                "{}  {}  {}  {}",
                ops::short_id(&rec.id),
                rec.location(),
                rec.size,
                ops::format_tags(&rec.tags)
            );
        }
    }
    EXIT_OK
}

/// Parse list selection from raw args: `--untagged` anywhere,
/// everything else is positional (at most one `category=value`,
/// and `--untagged` excludes filters).
fn parse_list_args(args: &[String]) -> Result<ops::ListFilter, String> {
    let mut untagged = false;
    let mut positionals = Vec::new();
    for a in args {
        if a == "--untagged" {
            untagged = true;
            continue;
        }
        positionals.push(a.clone());
    }
    if positionals.len() > 1 {
        return Err("list takes at most one category=value filter".into());
    }
    let filter = positionals.pop();
    match (untagged, filter) {
        (true, Some(_)) => Err("list: --untagged excludes filters".into()),
        (true, None) => Ok(ops::ListFilter::Untagged),
        (false, None) => Ok(ops::ListFilter::All),
        (false, Some(raw)) => match raw.split_once('=') {
            Some((cat, val)) if !cat.is_empty() && !val.is_empty() => Ok(ops::ListFilter::CatVal {
                cat: cat.into(),
                val: val.into(),
            }),
            _ => Err("list: filter must be category=value".into()),
        },
    }
}
