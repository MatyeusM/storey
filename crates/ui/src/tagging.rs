//! Tag-editor actions: set, suggestions, apply-all.
//!
//! Tag writes refresh the record list quietly; the view itself is
//! regenerable output, so no view rebuild happens here.

use crate::{
    state::{State, sync_detail},
    support,
    tags::{ExternalPlugin, TagSource, rank},
};

/// Write category values in always-set mode.
pub fn apply_tags(state: &mut State, category: &str, values: &str) {
    let cat = category.trim().to_string();
    let vals = support::split_values(values);
    if cat.is_empty() || vals.is_empty() {
        state.status_text = "storey error: enter a category and comma-separated values".into();
        return;
    }
    let id = state.selected_id.clone();
    let cwd = std::path::PathBuf::from(state.cwd.clone());
    let mut opened = match support::open_locked(&cwd) {
        Ok(o) => o,
        Err(detail) => {
            support::fail(state, "tag", &detail);
            return;
        }
    };
    let pos = match crate::tagging::tags_resolve(&opened.root, &opened.ix, &id) {
        Ok(pos) => pos,
        Err(detail) => {
            support::fail(state, "tag", &detail);
            return;
        }
    };
    let rec = &mut opened.ix.files[pos];
    storey_core::tags::apply_values(rec, &cat, vals, false);
    if opened.ix.save(&opened.root).is_err() {
        support::fail(state, "tag", "cannot save index");
        return;
    }
    state.status_text.clear();
    sync_records_quiet(state, &cwd);
    sync_detail(state);
}

/// Query every enabled plugin for the selected record.
pub fn do_suggest(state: &mut State) {
    let id = state.selected_id.clone();
    let root = std::path::PathBuf::from(state.root.clone());
    let record = state.records.iter().find(|r| r.id == id).cloned();
    let Some(record) = record else {
        state.status_text = "storey error: select a file first".into();
        return;
    };
    let all: Vec<_> = state
        .settings
        .enabled_plugins()
        .filter_map(|decl| ExternalPlugin::from_decl(decl).ok())
        .filter_map(|plugin| plugin.lookup(&record, &root).ok())
        .flatten()
        .collect();
    let ranked = rank(all);
    if ranked.is_empty() {
        state.status_text = "no suggestions".into();
        return;
    }
    state.suggestions = ranked;
    state.suggestions_json = support::suggestions_json(&state.suggestions.clone());
}

/// Read embedded file metadata for the selected record.
///
/// Local-only (never network): values are filtered view-safe, so the
/// `Apply all` path stays under the tag-time contract.
pub fn do_metadata(state: &mut State) {
    let id = state.selected_id.clone();
    let root = std::path::PathBuf::from(state.root.clone());
    let Some(record) = state.records.iter().find(|r| r.id == id).cloned() else {
        state.status_text = "storey error: select a file first".into();
        return;
    };
    let path = root.join(storey_core::library::from_slash(record.location()));
    let mut out = Vec::new();
    for item in storey_meta::suggest_for(&path) {
        // ponytail: drop view-unsafe values; upgrade to sanitize-on-read if real files hit this.
        if storey_core::tags::check_name(&item.category).is_ok()
            && storey_core::tags::check_name(&item.value).is_ok()
        {
            out.push(crate::tags::Suggestion {
                category: item.category,
                value: item.value,
                score: 1.0,
                source: "metadata".into(),
            });
        }
    }
    let ranked = rank(out);
    if ranked.is_empty() {
        state.status_text = "no metadata found".into();
        return;
    }
    state.suggestions = ranked;
    state.suggestions_json = support::suggestions_json(&state.suggestions.clone());
}

/// Merge every suggestion via grouped tag adds.
pub fn apply_suggestions(state: &mut State) {
    use std::collections::BTreeMap;
    let id = state.selected_id.clone();
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in &state.suggestions {
        let values = grouped.entry(item.category.clone()).or_default();
        if !values.contains(&item.value) {
            values.push(item.value.clone());
        }
    }
    if grouped.is_empty() {
        return;
    }
    let cwd = std::path::PathBuf::from(state.cwd.clone());
    for (cat, values) in &grouped {
        let mut opened = match support::open_locked(&cwd) {
            Ok(o) => o,
            Err(detail) => {
                support::fail(state, "tag", &detail);
                return;
            }
        };
        let pos = match crate::tagging::tags_resolve(&opened.root, &opened.ix, &id) {
            Ok(pos) => pos,
            Err(detail) => {
                support::fail(state, "tag", &detail);
                return;
            }
        };
        let rec = &mut opened.ix.files[pos];
        storey_core::tags::apply_values(rec, cat, values.clone(), true);
        if opened.ix.save(&opened.root).is_err() {
            support::fail(state, "tag", "cannot save index");
            return;
        }
    }
    state.suggestions.clear();
    state.suggestions_json = "[]".into();
    state.status_text.clear();
    sync_records_quiet(state, &cwd);
    sync_detail(state);
}

/// Reload `list` rows and categories without touching the status
/// line. Categories must refresh here: the folder-view [+] button
/// reads them, and the first-ever tags would otherwise stay hidden
/// until the next navigation.
fn sync_records_quiet(state: &mut State, cwd: &std::path::Path) {
    let Ok(root) = storey_core::library::find_root(cwd) else {
        return;
    };
    if let Ok(ix) = storey_core::index::Index::load(&root) {
        state.records = ix.files;
    }
    if let Ok(ix) = storey_core::index::Index::load(&root) {
        let cats = storey_core::tags::categories(&ix.files);
        state.categories = cats.clone();
        state.categories_json = support::str_list_json(&cats);
    }
    let query = state.filter_query.clone();
    let untagged = state.untagged_only;
    state.records_json = support::records_json(&state.records, &query, untagged);
}

/// Resolve a record id through core, mapping failures to details.
pub fn tags_resolve(
    root: &std::path::Path,
    ix: &storey_core::index::Index,
    id: &str,
) -> Result<usize, String> {
    match storey_core::tags::resolve_target(root, ix, id) {
        Ok(pos) => Ok(pos),
        Err(e) => match e.candidates {
            Some(c) => Err(format!("ambiguous id prefix {id:?}: {}", c.join(", "))),
            None => Err(e.error),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_without_tags_reports() {
        let dir = std::env::temp_dir().join(format!("storey-ui-meta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();
        let mut state = State::new(dir.display().to_string(), Default::default());
        state.root = dir.display().to_string();
        state.records.push(storey_core::index::Record {
            id: "ab".into(),
            display_name: "a.txt".into(),
            original_rel: "a.txt".into(),
            stored_rel: String::new(),
            size: 1,
            tags: Default::default(),
        });
        state.selected_id = "ab".into();
        do_metadata(&mut state);
        assert_eq!(state.status_text, "no metadata found");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
