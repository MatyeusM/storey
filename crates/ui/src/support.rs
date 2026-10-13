//! UI backend helpers: JSON builders plus small shared pieces.
//!
//! Everything here is Qt-free and unit-tested; `apps/gui` only
//! translates QStrings at the boundary. Filtering is UI-local:
//! outside a repo the query narrows entry names, inside it matches
//! file names or tag content.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use storey_core::index::Record;
use storey_core::log::LogEvent;

/// Opened library: root, held lock, loaded index.
pub struct Opened {
    pub root: PathBuf,
    pub _lock: storey_core::lock::Guard,
    pub ix: storey_core::index::Index,
}

/// Resolve cwd, claim the lock, load the index. Failures carry the
/// status detail for [`fail`].
pub fn open_locked(cwd: &Path) -> Result<Opened, String> {
    let root = storey_core::library::find_root(cwd).map_err(|e| e.to_string())?;
    let lock = storey_core::lock::acquire(&root)?;
    let ix = storey_core::index::Index::load(&root).map_err(|e| e.to_string())?;
    Ok(Opened {
        root,
        _lock: lock,
        ix,
    })
}

/// Record a failure in the status bar (no popup).
pub fn fail(state: &mut crate::state::State, op: &str, detail: &str) {
    let detail = detail.trim();
    let detail = if detail.is_empty() {
        "unknown error".to_string()
    } else {
        detail.to_string()
    };
    state.status_text = format!("storey error ({op}): {detail}");
}

/// Shorten an error for status lines (tested).
pub fn short_detail(message: &str) -> String {
    let short = crate::explore::short_err(message.to_string());
    if short.trim().is_empty() {
        "unknown error".to_string()
    } else {
        short
    }
}

/// Save a partial index after an abort; the detail goes to status.
pub fn emergency_save(root: &Path, ix: &storey_core::index::Index) -> String {
    if ix.save(root).is_ok() {
        "interrupted, index saved".to_string()
    } else {
        "interrupted (index may be stale)".to_string()
    }
}

/// Progress label plus fraction (-1 when unknown) for one tick.
pub fn progress_label(fallback: &str, ev: &LogEvent) -> Option<(String, f64)> {
    if !ev.is_progress() {
        return None;
    }
    let op = ev.op.clone().unwrap_or_else(|| fallback.to_string());
    Some(match ev.total {
        Some(t) if t > 0 => {
            let f = ev.done.min(t) as f64 / t as f64;
            (format!("indexing {op} {}%", (f * 100.0).round() as u64), f)
        }
        _ => (format!("indexing {op} {}…", ev.done), -1.0),
    })
}

/// Short 12-char id prefix for the detail view.
pub fn short_id(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

/// Sorted `cat=v1,v2 ...` display, `-` when empty.
pub fn display_tags(tags: &BTreeMap<String, Vec<String>>) -> String {
    if tags.is_empty() {
        return "-".to_string();
    }
    let mut cats: Vec<&String> = tags.keys().collect();
    cats.sort();
    cats.iter()
        .map(|c| format!("{c}={}", tags[*c].join(",")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Case-insensitive substring; empty needle matches everything.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Explorer rows for `cwd`, narrowed by `query` on the file name.
pub fn entries_json(cwd: &Path, query: &str) -> String {
    let q = query.trim();
    let rows: Vec<serde_json::Value> = crate::explore::list_dir(cwd)
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
pub fn records_json(records: &[Record], query: &str, untagged: bool) -> String {
    let rows: Vec<&Record> = records
        .iter()
        .filter(|r| record_matches(r, query, untagged))
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

/// Selected record plus tag summary, or `null` when missing.
pub fn detail_json(records: &[Record], id: &str) -> String {
    match records.iter().find(|r| r.id == id) {
        Some(r) => serde_json::json!({
            "id": r.id,
            "shortId": short_id(&r.id),
            "displayName": r.display_name,
            "size": r.size,
            "location": r.location(),
            "tagsText": display_tags(&r.tags),
            "tags": r.tags,
        })
        .to_string(),
        None => "null".to_string(),
    }
}

/// Ranked suggestions serialized for the tag editor.
pub fn suggestions_json(suggestions: &[crate::tags::Suggestion]) -> String {
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
pub fn str_list_json(items: &[String]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

/// Split a values field on `,`, trimming and dropping empties.
pub fn split_values(raw: &str) -> Vec<String> {
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
        let dir = std::env::temp_dir().join(format!("ui-entries-{}", std::process::id()));
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

    #[test]
    fn progress_labels_tick() {
        let ev = storey_core::log::LogEvent {
            level: "progress".into(),
            msg: None,
            op: Some("hash".into()),
            done: 1,
            total: Some(4),
            path: "a".into(),
        };
        assert_eq!(
            progress_label("hash", &ev),
            Some(("indexing hash 25%".into(), 0.25))
        );
        let info = storey_core::log::LogEvent {
            level: "info".into(),
            ..ev.clone()
        };
        assert_eq!(progress_label("hash", &info), None);
    }
}
