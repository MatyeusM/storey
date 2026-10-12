//! `view` reports: dry previews, block notices, summaries.

use std::collections::BTreeMap;

use crate::{EXIT_FAILURE, EXIT_OK, emit};
use storey_core::materialize::{self, ViewReport};

pub(crate) fn dry_view(
    as_json: bool,
    by: &[String],
    moves: &BTreeMap<String, Option<String>>,
    links: &[storey_core::views::Link],
    mut rep: ViewReport,
) -> i32 {
    rep.moves = moves
        .iter()
        .map(|(k, v)| (k.clone(), v.clone().unwrap_or_default()))
        .collect();
    rep.plan = materialize::link_paths(links);
    emit("view", as_json, &rep);
    print_summary(
        as_json,
        by,
        rep.moves.len(),
        0,
        rep.links,
        0,
        rep.unsorted,
        true,
    );
    EXIT_OK
}

pub(crate) fn blocked(as_json: bool, rep: &ViewReport, extra: &[String]) -> i32 {
    emit("view", as_json, rep);
    if !as_json {
        eprintln!("storey: view: blocked (use --force to destroy/overwrite):");
        for u in &rep.untracked {
            eprintln!("  untracked: {u}");
        }
        for line in extra {
            eprintln!("{line}");
        }
    }
    EXIT_FAILURE
}

pub(crate) fn orphan_lines(orphans: &[String]) -> Vec<String> {
    orphans
        .iter()
        .map(|o| format!("  orphaned store file: {o}"))
        .collect()
}

pub(crate) fn print_summary(
    as_json: bool,
    by: &[String],
    moved: usize,
    purged: usize,
    links: usize,
    link_errors: usize,
    unsorted: usize,
    dry: bool,
) {
    if as_json {
        return;
    }
    let suffix = if dry { " (dry run)" } else { "" };
    println!(
        "view by {}{}\nmoved: {}\npurged (duplicates): {}\nlinks: {}\nlink errors: {}\nunsorted: {}",
        by.join(","),
        suffix,
        moved,
        purged,
        links,
        link_errors,
        unsorted
    );
}
