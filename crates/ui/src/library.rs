//! Library actions: shallow filter, untagged toggle, file-view
//! switch, view-tag writes, and record/file selection.

use crate::{
    state::{State, sync_detail},
    support,
};

/// Narrow listings on every keystroke.
pub fn set_filter(state: &mut State, query: String) {
    state.filter_query = query;
    crate::nav::clear_status(state);
    crate::state::sync_all(state);
}

/// Flip the untagged lens (forcing the file view).
pub fn toggle_untagged(state: &mut State) {
    state.untagged_only = !state.untagged_only;
    if state.untagged_only {
        // Untagged results only exist in the file view.
        state.file_view = true;
    }
    crate::nav::clear_status(state);
    crate::state::sync_all(state);
}

/// Switch files/folders; leaving for folders drops the lens.
pub fn show_file_view(state: &mut State, file: bool) {
    state.file_view = file;
    if !file && state.untagged_only {
        // Leaving the untagged results: restore the search lens.
        state.untagged_only = false;
        crate::nav::clear_status(state);
        crate::state::sync_all(state);
    }
}

/// Materialize a new view key order: covers breadcrumb add and
/// per-pill remove from QML in one call.
pub fn set_view_tags(state: &mut State, tags_json: &str) {
    let tags = parse_tags(tags_json);
    let cwd = std::path::PathBuf::from(state.cwd.clone());
    let mut opened = match support::open_locked(&cwd) {
        Ok(o) => o,
        Err(detail) => {
            support::fail(state, "view", &detail);
            return;
        }
    };
    let root = opened.root.clone();
    let (untracked, orphans) = match storey_core::materialize::scan_blockers(&root, &mut opened.ix)
    {
        Ok(v) => v,
        Err(e) => {
            support::fail(state, "view", &support::short_detail(&e.to_string()));
            return;
        }
    };
    if !untracked.is_empty() || !orphans.is_empty() {
        support::fail(state, "view", &blocked_detail(&untracked, &orphans));
        return;
    }
    if let Err(detail) = apply_view_tags(&root, &mut opened.ix, &tags) {
        support::fail(state, "view", &detail);
        return;
    }
    state.status_text.clear();
    crate::state::sync_all(state);
}

/// Apply moves, wipe, links, and record the key order.
fn apply_view_tags(
    root: &std::path::Path,
    ix: &mut storey_core::index::Index,
    tags: &[String],
) -> Result<(), String> {
    use std::sync::atomic::AtomicBool;
    let logger = storey_core::log::Logger::new(false, true);
    let calm = AtomicBool::new(false);
    let moves = storey_core::materialize::compute_moves(ix);
    let links = storey_core::materialize::planned_links(ix, &moves, tags);
    let ctx = storey_core::materialize::ExecCtx {
        root,
        by: tags,
        untracked: vec![],
        orphans: vec![],
        moves: &moves,
        links: &links,
        logger: &logger,
        abort: &calm,
    };
    match storey_core::materialize::exec_view(ix, ctx) {
        Ok(storey_core::flows::FlowDone::Done(_)) => Ok(()),
        Ok(storey_core::flows::FlowDone::Aborted) => Err(support::emergency_save(root, ix)),
        Err(e) => Err(support::short_detail(&e.to_string())),
    }
}

/// Block detail naming untracked files and orphans.
fn blocked_detail(untracked: &[String], orphans: &[String]) -> String {
    let mut parts = vec!["blocked (use --force to destroy/overwrite)".to_string()];
    for u in untracked {
        parts.push(format!("untracked: {u}"));
    }
    for o in orphans {
        parts.push(format!("orphaned store file: {o}"));
    }
    parts.join(", ")
}

/// Focus a record and clear stale suggestions.
pub fn select_record(state: &mut State, id: String) {
    state.selected_id = id;
    state.suggestions_json = "[]".into();
    state.suggestions.clear();
    sync_detail(state);
}

/// Resolve a link path to its record id, then select.
pub fn select_file(state: &mut State, path: &str) {
    state.selected_id = resolve_id(state, path);
    state.suggestions_json = "[]".into();
    state.suggestions.clear();
    sync_detail(state);
}

/// Resolve an explorer path to a record id via display name match.
fn resolve_id(state: &State, target: &str) -> String {
    let base = std::path::Path::new(target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    state
        .records
        .iter()
        .find(|r| r.display_name == base)
        .map(|r| r.id.clone())
        .unwrap_or_default()
}

/// Parse the QML view-tag array; garbage becomes an empty view.
fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_arrays() {
        assert_eq!(parse_tags(r#"["a","b"]"#), vec!["a", "b"]);
        assert!(parse_tags("garbage").is_empty());
    }
}
