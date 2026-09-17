//! Synchronous state sync plus async completion helpers.
//!
//! [`sync_all`] re-reads places, explorer rows, `status`,
//! categories, and records after every navigation or mutation.
//! The status bar stays quiet when everything is normal: only
//! not-a-library, indexing progress, and `storey error:` lines show.

use core::pin::Pin;
use std::path::Path;

use cxx_qt::CxxQtType;
use storey_shared::cli::CliError;
use storey_shared::model::UpdateReport;

use crate::{places, store::ffi, support};

/// Re-read places, status, entries, records, and detail.
pub(crate) fn sync_all(obj: &mut Pin<&mut ffi::StoreyLibrary>) {
    sync_cwd(obj);
    sync_places(obj);
    let cwd = support::path_of(obj.as_ref().cwd());
    sync_status(obj, &cwd);
    sync_entries_records(obj, &cwd);
    sync_detail(obj);
}

/// Canonicalize the cwd property in place.
fn sync_cwd(obj: &mut Pin<&mut ffi::StoreyLibrary>) {
    let raw = support::path_of(obj.as_ref().cwd());
    let done = storey_shared::explore::canonical(&raw);
    obj.as_mut()
        .set_cwd(support::qstr(&done.display().to_string()));
}

/// Refresh the side-pane quick places.
fn sync_places(obj: &mut Pin<&mut ffi::StoreyLibrary>) {
    let places = places::quick_places();
    obj.as_mut()
        .set_places_json(support::qstr(&places::places_json(&places)));
}

/// Poll `status` (+ categories) and update repo properties.
fn sync_status(obj: &mut Pin<&mut ffi::StoreyLibrary>, cwd: &Path) {
    let client = support::client_of(obj.as_ref().storey_bin());
    match client.status(cwd, |_| {}) {
        Ok(status) => {
            obj.as_mut().set_is_repo(true);
            obj.as_mut().set_root(support::qstr(&status.root));
            obj.as_mut().rust_mut().view = status.view.clone();
            obj.as_mut()
                .set_view_json(support::qstr(&support::str_list_json(&status.view)));
            if support::s(obj.as_ref().status_text()).starts_with("storey error:") {
                return;
            }
            obj.as_mut().set_status_text(support::qstr(""));
            match client.categories(cwd, |_| {}) {
                Ok(cats) => {
                    obj.as_mut().rust_mut().categories = cats.clone();
                    obj.as_mut()
                        .set_categories_json(support::qstr(&support::str_list_json(&cats)));
                }
                Err(e) => fail(obj, "status", &e),
            }
        }
        Err(e) if e.code == Some(2) => {
            obj.as_mut().set_is_repo(false);
            obj.as_mut().set_root(support::qstr(""));
            obj.as_mut().rust_mut().view.clear();
            obj.as_mut().rust_mut().categories.clear();
            obj.as_mut().set_view_json(support::qstr("[]"));
            obj.as_mut().set_categories_json(support::qstr("[]"));
            if !support::s(obj.as_ref().status_text()).starts_with("storey error:") {
                obj.as_mut()
                    .set_status_text(support::qstr("not a storey directory"));
            }
        }
        Err(e) => fail(obj, "status", &e),
    }
}

/// Reload `list` rows, repaint filtered models, and recount.
fn sync_entries_records(obj: &mut Pin<&mut ffi::StoreyLibrary>, cwd: &Path) {
    let query = support::s(obj.as_ref().filter_query());
    let untagged = *obj.as_ref().untagged_only();
    let is_repo = *obj.as_ref().is_repo();
    obj.as_mut()
        .set_entries_json(support::qstr(&support::entries_json(cwd, &query)));
    if is_repo {
        let client = support::client_of(obj.as_ref().storey_bin());
        match client.list(cwd, |_| {}) {
            Ok(records) => obj.as_mut().rust_mut().records = records,
            Err(e) => {
                obj.as_mut().rust_mut().records.clear();
                if e.code != Some(2) {
                    fail(obj, "list", &e);
                }
            }
        }
    } else {
        obj.as_mut().rust_mut().records.clear();
    }
    paint_filtered(obj, cwd, &query, untagged, is_repo);
}

/// Repaint filtered JSON plus folder/file counts and total size.
fn paint_filtered(
    obj: &mut Pin<&mut ffi::StoreyLibrary>,
    cwd: &Path,
    query: &str,
    untagged: bool,
    is_repo: bool,
) {
    let entries = storey_shared::explore::list_dir(cwd);
    let q = query.trim().to_lowercase();
    let mut folders = 0i32;
    let mut files = 0i32;
    let mut bytes = 0u64;
    for e in &entries {
        if !e.name.to_lowercase().contains(&q) {
            continue;
        }
        if e.is_dir {
            folders += 1;
        } else if !is_repo {
            files += 1;
            bytes += e.size;
        }
    }
    if is_repo {
        let rust = obj.rust();
        let mut shown = 0i32;
        let mut total = 0u64;
        for r in &rust.records {
            let keep = if untagged {
                r.tags.is_empty()
            } else if q.is_empty() {
                true
            } else {
                let in_name = r.display_name.to_lowercase().contains(&q);
                let in_tags = r
                    .tags
                    .values()
                    .flatten()
                    .any(|v| v.to_lowercase().contains(&q))
                    || r.tags.keys().any(|c| c.to_lowercase().contains(&q));
                in_name || in_tags
            };
            if keep {
                shown += 1;
                total += r.size;
            }
        }
        files = shown;
        bytes = total;
        let json = support::records_json(&rust.records, query, untagged);
        obj.as_mut().set_records_json(support::qstr(&json));
    } else {
        obj.as_mut().set_records_json(support::qstr("[]"));
    }
    obj.as_mut().set_folder_count(folders);
    obj.as_mut().set_file_count(files);
    obj.as_mut()
        .set_size_text(support::qstr(&places::human_size(bytes)));
}

/// Repaint the tag-editor detail for the current selection.
pub(crate) fn sync_detail(obj: &mut Pin<&mut ffi::StoreyLibrary>) {
    let id = support::s(obj.as_ref().selected_id());
    let (json, valid) = {
        let rust = obj.rust();
        let json = support::detail_json(&rust.records, &id);
        (json, rust.records.iter().any(|r| r.id == id))
    };
    obj.as_mut().set_detail_json(support::qstr(&json));
    if !valid {
        obj.as_mut().set_selected_id(support::qstr(""));
    }
}

/// Record a CLI failure in the status bar (no popup).
pub(crate) fn fail(obj: &mut Pin<&mut ffi::StoreyLibrary>, op: &str, e: &CliError) {
    // Only spawn failures mean a missing binary: parse errors
    // (code None from typed decoding) must show their own text.
    let detail = if e.code.is_none() && e.message.starts_with("spawning") {
        let bin = support::s(obj.as_ref().storey_bin());
        format!("no storey binary at {bin}")
    } else {
        storey_shared::explore::short_err(e.to_string())
    };
    let detail = detail.trim();
    let detail = if detail.is_empty() {
        "unknown error".to_string()
    } else {
        detail.to_string()
    };
    obj.as_mut()
        .set_status_text(support::qstr(&format!("storey error ({op}): {detail}")));
}

/// Claim the busy flag; refuse with a status line when an op runs.
pub(crate) fn take_busy(mut obj: Pin<&mut ffi::StoreyLibrary>) -> bool {
    if *obj.as_ref().busy() {
        obj.as_mut().set_status_text(support::qstr(
            "storey error: busy — wait for the running op",
        ));
        return true;
    }
    obj.as_mut().set_busy(true);
    obj.as_mut().set_progress_label(support::qstr("indexing…"));
    obj.as_mut().set_progress_fraction(-1.0);
    false
}

/// Forward a hashing progress event to the status bar.
pub(crate) fn send_tick(
    thread: &cxx_qt::CxxQtThread<ffi::StoreyLibrary>,
    fallback: &str,
    ev: &storey_shared::cli::LogEvent,
) {
    if !ev.is_progress() {
        return;
    }
    let op = ev.op.clone().unwrap_or_else(|| fallback.to_string());
    let done = ev.done.unwrap_or(0);
    let (label, fraction) = progress_parts(&op, done, ev.total);
    let _ = thread.queue(move |mut obj| {
        obj.as_mut().set_progress_label(support::qstr(&label));
        obj.as_mut().set_progress_fraction(fraction);
    });
}

/// TUI-style progress label plus a fraction (-1 when unknown).
fn progress_parts(op: &str, done: u64, total: Option<u64>) -> (String, f64) {
    match total {
        Some(t) if t > 0 => {
            let f = done.min(t) as f64 / t as f64;
            (format!("indexing {op} {}%", (f * 100.0).round() as u64), f)
        }
        _ => (format!("indexing {op} {done}…"), -1.0),
    }
}

/// Apply an `update` outcome, then refresh into the rescanned repo.
/// Problem exits (1) still carry a report: show its summary instead
/// of an error. Summary goes last: a plain refresh would clear it.
pub(crate) fn finish_update(
    obj: &mut Pin<&mut ffi::StoreyLibrary>,
    result: Result<UpdateReport, CliError>,
) {
    obj.as_mut().set_busy(false);
    obj.as_mut().set_progress_label(support::qstr(""));
    obj.as_mut().set_progress_fraction(-1.0);
    sync_all(obj);
    match result {
        Ok(report) => obj
            .as_mut()
            .set_status_text(support::qstr(&report.summary())),
        Err(e) => match partial_report(&e) {
            Some(report) => obj
                .as_mut()
                .set_status_text(support::qstr(&report.summary())),
            None => fail(obj, "update", &e),
        },
    }
}

/// Update report from a problem exit, when the envelope survived.
fn partial_report(e: &CliError) -> Option<UpdateReport> {
    let data = e.envelope.clone()?.data;
    serde_json::from_value(data).ok()
}

/// Apply an `init` outcome, then refresh into the new repo.
pub(crate) fn finish_init(obj: &mut Pin<&mut ffi::StoreyLibrary>, result: Result<(), CliError>) {
    obj.as_mut().set_busy(false);
    obj.as_mut().set_progress_label(support::qstr(""));
    obj.as_mut().set_progress_fraction(-1.0);
    match result {
        Ok(()) => obj.as_mut().set_status_text(support::qstr("")),
        Err(e) => fail(obj, "init", &e),
    }
    sync_all(obj);
}
