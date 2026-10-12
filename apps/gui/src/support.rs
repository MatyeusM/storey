//! Shared GUI backend helpers: Qt-free JSON builders.
//!
//! The `ffi::StoreyLibrary` impl blocks in `nav`, `library`,
//! `tagging`, and `plugins` call these; Qt setters stay at the call
//! sites. Filtering is GUI-local: outside a repo the query narrows
//! entry names, inside it matches file names or tag content.
//!
//! Places, sizes, and explorer-open live in `places`; plugin rows in
//! `plugins`.

use std::path::{Path, PathBuf};

use cxx_qt_lib::QString;
use storey_shared::{Client, model::Record};

/// Wrap a string for a `QString` property.
pub(crate) fn qstr(value: &str) -> QString {
    QString::from(value)
}

/// Read a `QString` property as an owned string.
pub(crate) fn s(value: &QString) -> String {
    value.to_string()
}

/// CLI driver for the configured binary.
pub(crate) fn client_of(bin: &QString) -> Client {
    Client::new(PathBuf::from(s(bin)))
}

/// Property text as a filesystem path.
pub(crate) fn path_of(value: &QString) -> PathBuf {
    PathBuf::from(s(value))
}

/// Case-insensitive substring; empty needle matches everything.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Explorer rows for `cwd`, narrowed by `query` on the file name.
pub(crate) fn entries_json(cwd: &Path, query: &str) -> String {
    let q = query.trim();
    let rows: Vec<serde_json::Value> = storey_shared::explore::list_dir(cwd)
        .iter()
        .filter(|e| contains_ci(&e.name, q))
        .map(|e| {
            serde_json::json!({
                "name": e.name,
                "path": e.path.display().to_string(),
                "isDir": e.is_dir,
                "size": e.size,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// True when a record matches the shallow search (name or tags).
fn record_matches(record: &Record, query: &str, untagged_only: bool) -> bool {
    if untagged_only {
        return record.tags.is_empty();
    }
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    if contains_ci(&record.display_name, q) {
        return true;
    }
    record.tags.values().flatten().any(|v| contains_ci(v, q))
        || record.tags.keys().any(|c| contains_ci(c, q))
}

/// Visible records under the shallow search, serialized for QML.
pub(crate) fn records_json(records: &[Record], query: &str, untagged: bool) -> String {
    let rows: Vec<&Record> = records
        .iter()
        .filter(|r| record_matches(r, query, untagged))
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// Selected record plus tag summary, or `null` when missing.
pub(crate) fn detail_json(records: &[Record], id: &str) -> String {
    match records.iter().find(|r| r.id == id) {
        Some(r) => serde_json::json!({
            "id": r.id,
            "shortId": r.short_id(),
            "displayName": r.display_name,
            "size": r.size,
            "location": r.location(),
            "tagsText": r.display_tags(),
            "tags": r.tags,
        })
        .to_string(),
        None => "null".to_string(),
    }
}

/// Ranked suggestions serialized for the tag editor.
pub(crate) fn suggestions_json(suggestions: &[storey_shared::tags::Suggestion]) -> String {
    let rows: Vec<serde_json::Value> = suggestions
        .iter()
        .map(|item| {
            serde_json::json!({
                "category": item.category,
                "value": item.value,
                "score": item.score,
                "source": item.source,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// String list serialized for QML (`JSON.parse` at the view).
pub(crate) fn str_list_json(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

/// Split a values field on `,`, trimming and dropping empties.
pub(crate) fn split_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, name: &str, tags: &[(&str, &[&str])]) -> Record {
        let tags = tags
            .iter()
            .map(|(c, vs)| (c.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect();
        Record {
            id: id.into(),
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size: 7,
            tags,
        }
    }

    #[test]
    fn values_split() {
        assert_eq!(split_values("a, b,, c "), vec!["a", "b", "c"]);
        assert!(split_values(" , ").is_empty());
    }

    #[test]
    fn shallow_search_matches_names_or_tags() {
        let records = vec![
            record("a", "Concert.mkv", &[("genre", &["rock"])]),
            record("b", "notes.txt", &[]),
        ];
        let shown = |q: &str, u: bool| {
            let v: serde_json::Value = serde_json::from_str(&records_json(&records, q, u)).unwrap();
            v.as_array().unwrap().len()
        };
        assert_eq!(shown("concert", false), 1);
        assert_eq!(shown("rock", false), 1);
        assert_eq!(shown("genre", false), 1);
        assert_eq!(shown("", true), 1);
        assert_eq!(shown("zzz", false), 0);
    }

    #[test]
    fn entries_hide_store_and_dotfiles() {
        let dir = std::env::temp_dir().join(format!("storey-gui-entries-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join(".files")).unwrap();
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        std::fs::write(dir.join(".index.json"), "{}").unwrap();
        let rows: serde_json::Value = serde_json::from_str(&entries_json(&dir, "")).unwrap();
        let names: Vec<_> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["sub", "b.txt"]);
        let filtered: serde_json::Value = serde_json::from_str(&entries_json(&dir, "b.t")).unwrap();
        assert_eq!(filtered.as_array().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
