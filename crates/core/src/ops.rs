//! Library operations behind CLI commands. Pure flows over the
//! index plus filesystem effects; the `apps/cli` binary only parses
//! arguments and prints the resulting reports.

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::index::{Index, Record};
use crate::library;
use crate::views;

/// Read-only overview: where the library is, how much is indexed,
/// stored, and waiting on disk. No hashing, no mutation.
#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub root: String,
    pub indexed: usize,
    pub stored: usize,
    pub untracked: usize,
    pub view: Vec<String>,
}

/// Compute the status overview for the library at root.
pub fn status_at(root: &Path) -> Result<StatusReport> {
    let ix = Index::load(root)?;
    let tracked = library::current_locations(&ix.files);
    let artifacts = views::plan_set(&ix.files, &ix.view.by);
    let untracked = library::find_untracked(root, &tracked, &artifacts)?;
    Ok(StatusReport {
        root: root.display().to_string(),
        indexed: ix.files.len(),
        stored: ix.files.iter().filter(|r| r.stored()).count(),
        untracked: untracked.len(),
        view: ix.view.by.clone(),
    })
}

/// Compute the status overview for the library containing cwd.
pub fn status(cwd: &Path) -> Result<StatusReport> {
    let root = library::find_root(cwd)?;
    status_at(&root)
}

/// Record filter for `list`: everything, untagged only, or one
/// exact `category=value` match.
pub enum ListFilter {
    All,
    Untagged,
    CatVal { cat: String, val: String },
}

/// Records matching the filter, in index order.
pub fn list_records(files: &[Record], filter: &ListFilter) -> Vec<Record> {
    files
        .iter()
        .filter(|rec| match filter {
            ListFilter::All => true,
            ListFilter::Untagged => rec.tags.is_empty(),
            ListFilter::CatVal { cat, val } => rec
                .tags
                .get(cat)
                .map(|vals| vals.iter().any(|v| v == val))
                .unwrap_or(false),
        })
        .cloned()
        .collect()
}

/// Sorted `cat=v1,v2 ...` display, `-` when empty.
pub fn format_tags(tags: &std::collections::BTreeMap<String, Vec<String>>) -> String {
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

/// Short 12-char id prefix for tables.
pub fn short_id(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_match_go() {
        let recs = vec![
            Record {
                id: "a".into(),
                display_name: "a".into(),
                original_rel: "a".into(),
                stored_rel: String::new(),
                size: 1,
                tags: [("g".to_string(), vec!["rock".to_string()])]
                    .into_iter()
                    .collect(),
            },
            Record {
                id: "b".into(),
                display_name: "b".into(),
                original_rel: "b".into(),
                stored_rel: String::new(),
                size: 1,
                tags: Default::default(),
            },
        ];
        assert_eq!(list_records(&recs, &ListFilter::All).len(), 2);
        assert_eq!(list_records(&recs, &ListFilter::Untagged).len(), 1);
        let f = ListFilter::CatVal {
            cat: "g".into(),
            val: "rock".into(),
        };
        assert_eq!(list_records(&recs, &f).len(), 1);
        assert_eq!(format_tags(&recs[0].tags), "g=rock");
        assert_eq!(format_tags(&recs[1].tags), "-");
    }

    #[test]
    fn status_reads_a_library() {
        let dir = std::env::temp_dir().join(format!("storey-core-ops-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        Index::new().save(&dir).unwrap();
        let rep = status(&dir).unwrap();
        assert_eq!(rep.indexed, 0);
        assert_eq!(rep.untracked, 1);
        assert!(rep.view.is_empty());
        assert!(library::find_root(&dir).is_ok());
        assert!(library::find_root(&std::env::temp_dir().join("storey-core-ops-nope")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
