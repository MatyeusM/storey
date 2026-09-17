//! Index record types for display.
//!
//! These mirror the CLI `--json` outputs (`docs/CLI-CONTRACT.md`) so
//! both UIs deserialize the same structs. Read-only: the UIs never
//! write the index, they only render what the CLI reports.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One indexed file, as reported by `storey list --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "originalRel")]
    pub original_rel: String,
    #[serde(rename = "storedRel", default)]
    pub stored_rel: String,
    pub size: u64,
    #[serde(default)]
    pub tags: BTreeMap<String, Vec<String>>,
}

impl Record {
    /// True once `view` has ingested the object into `.files/`.
    pub fn is_stored(&self) -> bool {
        !self.stored_rel.is_empty()
    }

    /// Current path relative to the library root.
    pub fn location(&self) -> &str {
        if self.is_stored() {
            &self.stored_rel
        } else {
            &self.original_rel
        }
    }

    /// Short id for tables (first 12 hex chars, like the CLI).
    pub fn short_id(&self) -> &str {
        &self.id[..self.id.len().min(12)]
    }

    /// Human tag summary (`actor=X,Y date=2020`, `-` when untagged).
    pub fn display_tags(&self) -> String {
        format_tags(&self.tags)
    }
}

/// Read-only overview from `storey status --json`: safe to poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub root: String,
    pub indexed: u64,
    pub stored: u64,
    pub untracked: u64,
    #[serde(default)]
    pub view: Vec<String>,
}

impl Status {
    /// Ordered view key list (`date,actor`), `(none)` when empty.
    pub fn display_view(&self) -> String {
        if self.view.is_empty() {
            "(none)".to_string()
        } else {
            self.view.join(",")
        }
    }
}

/// Result of `storey view --tags=... --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewReport {
    #[serde(default)]
    pub by: Vec<String>,
    #[serde(default)]
    pub moved: u64,
    #[serde(default)]
    pub purged: Vec<String>,
    #[serde(default)]
    pub untracked: Vec<String>,
    #[serde(default)]
    pub links: u64,
    #[serde(default)]
    pub unsorted: u64,
    #[serde(default, rename = "linkErrors")]
    pub link_errors: Vec<String>,
    #[serde(default)]
    pub orphans: Vec<String>,
}

/// Result of `storey update [--verify] --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateReport {
    #[serde(default)]
    pub added: Vec<String>,
    #[serde(default)]
    pub purged: Vec<String>,
    #[serde(default)]
    pub adopted: Vec<String>,
    #[serde(default)]
    pub removed: Vec<String>,
    #[serde(default)]
    pub reverified: u64,
    #[serde(default)]
    pub problems: Vec<String>,
}

impl UpdateReport {
    /// One-line summary for notices (`3 added · 1 removed · 2 problems`).
    pub fn summary(&self) -> String {
        let mut parts = vec![
            format!("{} added", self.added.len()),
            format!("{} removed", self.removed.len()),
        ];
        if self.reverified > 0 {
            parts.push(format!("{} re-hashed", self.reverified));
        }
        if !self.problems.is_empty() {
            parts.push(format!("{} problems", self.problems.len()));
        }
        parts.join(" · ")
    }
}

/// Result of `storey restore [--force] [--dry-run] --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    #[serde(default)]
    pub restored: u64,
    #[serde(default)]
    pub skipped: u64,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub problems: Vec<String>,
    #[serde(default)]
    pub untracked: Vec<String>,
    #[serde(default)]
    pub moves: BTreeMap<String, String>,
}

impl RestoreReport {
    /// One-line summary for notices (`2 restored · 1 problems`).
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("{} restored", self.restored)];
        if self.skipped > 0 {
            parts.push(format!("{} skipped", self.skipped));
        }
        if !self.problems.is_empty() {
            parts.push(format!("{} problems", self.problems.len()));
        }
        parts.join(" · ")
    }

    /// True when restore refused to run without `--force`.
    pub fn blocked(&self) -> bool {
        !self.untracked.is_empty() || !self.conflicts.is_empty()
    }

    /// Short blocker list for notices (`2 untracked · 1 conflicts`).
    pub fn blockers(&self) -> String {
        let mut parts = Vec::new();
        if !self.untracked.is_empty() {
            parts.push(format!("{} untracked", self.untracked.len()));
        }
        if !self.conflicts.is_empty() {
            parts.push(format!("{} conflicts", self.conflicts.len()));
        }
        parts.join(" · ")
    }
}

/// Result of `storey mv <target> <new-name> --json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MvReport {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub old: String,
    #[serde(default)]
    pub new: String,
}

/// Render `actor=X,Y date=2020` with sorted categories, `-` when empty.
pub fn format_tags(tags: &BTreeMap<String, Vec<String>>) -> String {
    if tags.is_empty() {
        return "-".to_string();
    }
    let mut cats: Vec<&String> = tags.keys().collect();
    cats.sort();
    cats.iter()
        .map(|cat| format!("{cat}={}", tags[*cat].join(",")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse the `data` array of a `list` envelope into records.
pub fn parse_list(data: &serde_json::Value) -> Result<Vec<Record>> {
    serde_json::from_value(data.clone()).context("parsing storey list records")
}

/// Parse the `data` object of a `status` envelope.
pub fn parse_status(data: &serde_json::Value) -> Result<Status> {
    serde_json::from_value(data.clone()).context("parsing storey status")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_parses_and_locates() {
        let data = serde_json::json!([
            {"id": "ab", "displayName": "a.mkv", "originalRel": "films/a.mkv",
             "storedRel": ".files/a.mkv", "size": 3, "tags": {"actor": ["X"]}},
            {"id": "cd", "displayName": "b.mkv", "originalRel": "b.mkv",
             "storedRel": "", "size": 1, "tags": {}},
        ]);
        let records = parse_list(&data).unwrap();
        assert!(records[0].is_stored());
        assert_eq!(records[0].location(), ".files/a.mkv");
        assert_eq!(records[0].short_id(), "ab");
        assert!(!records[1].is_stored());
        assert_eq!(records[1].location(), "b.mkv");
    }

    #[test]
    fn status_parses_and_formats() {
        let data = serde_json::json!({
            "root": "/lib", "indexed": 2, "stored": 1,
            "untracked": 0, "view": ["date"],
        });
        let status = parse_status(&data).unwrap();
        assert_eq!(status.display_view(), "date");
        let empty = Status {
            root: "/lib".into(),
            indexed: 0,
            stored: 0,
            untracked: 0,
            view: vec![],
        };
        assert_eq!(empty.display_view(), "(none)");
    }

    #[test]
    fn tags_format_matches_cli() {
        let mut tags = BTreeMap::new();
        assert_eq!(format_tags(&tags), "-");
        tags.insert("actor".into(), vec!["X".into(), "Y".into()]);
        tags.insert("date".into(), vec!["2020".into()]);
        assert_eq!(format_tags(&tags), "actor=X,Y date=2020");
    }

    #[test]
    fn update_summary_counts() {
        let rep = UpdateReport {
            added: vec!["a".into()],
            purged: vec![],
            adopted: vec![],
            removed: vec!["b".into()],
            reverified: 0,
            problems: vec![],
        };
        assert_eq!(rep.summary(), "1 added · 1 removed");
    }
}
