//! Record filter for the GUI: narrow by name, tags, or lack of tags.
//!
//! Pure matching over [`crate::model::Record`]; the GUI keeps
//! its own panel chrome. Filtering
//! is live — every keystroke re-narrows — and `Untagged` ignores the
//! query.

use crate::model::Record;

/// Which subset of records the filter keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterKind {
    /// Case-insensitive substring of the file name.
    #[default]
    Filename,
    /// Bare value (`rock`) or `category=value`, substring, case-insensitive.
    Tags,
    /// Records with no tags at all; the query is ignored.
    Untagged,
}

impl FilterKind {
    /// Parse the GUI kind name; unknown yields filename.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "tags" | "tag" => Self::Tags,
            "untagged" => Self::Untagged,
            _ => Self::Filename,
        }
    }

    /// Stable lowercase name for QML round-trips.
    pub fn name(self) -> &'static str {
        match self {
            Self::Filename => "filename",
            Self::Tags => "tags",
            Self::Untagged => "untagged",
        }
    }
}

/// Active kind plus the shared query buffer.
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub open: bool,
    pub kind: FilterKind,
    pub query: String,
}

impl FilterState {
    /// True when the list is actually narrowed.
    pub fn active(&self) -> bool {
        match self.kind {
            FilterKind::Untagged => true,
            _ => !self.query.trim().is_empty(),
        }
    }

    /// Cycle filename → tags → untagged (Tab) or backwards.
    pub fn cycle(&mut self, delta: i32) {
        const ORDER: [FilterKind; 3] =
            [FilterKind::Filename, FilterKind::Tags, FilterKind::Untagged];
        let cur = ORDER.iter().position(|&k| k == self.kind).unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(ORDER.len() as i32);
        self.kind = ORDER[next as usize];
    }

    /// Drop the query, reset to filename mode, and close.
    pub fn clear(&mut self) {
        self.query.clear();
        self.kind = FilterKind::Filename;
        self.open = false;
    }
}

/// Indices of `records` passing `filter`, in order.
pub fn visible_indices(records: &[Record], filter: &FilterState) -> Vec<usize> {
    records
        .iter()
        .enumerate()
        .filter(|(_, r)| matches(r, filter))
        .map(|(i, _)| i)
        .collect()
}

/// Short filter description for titles, if narrowing.
pub fn filter_summary(filter: &FilterState) -> Option<String> {
    if !filter.active() {
        return None;
    }
    match filter.kind {
        FilterKind::Untagged => Some("untagged".to_string()),
        FilterKind::Filename => Some(format!("name:{}", filter.query.trim())),
        FilterKind::Tags => Some(format!("tags:{}", filter.query.trim())),
    }
}

/// One record against the active kind (inactive kinds match all).
fn matches(record: &Record, filter: &FilterState) -> bool {
    match filter.kind {
        FilterKind::Filename => contains_ci(&record.display_name, filter.query.trim()),
        FilterKind::Tags => matches_tags(record, filter.query.trim()),
        FilterKind::Untagged => record.tags.is_empty(),
    }
}

/// Substring match; an empty needle matches everything.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Tag match: `cat=val` narrows both sides, bare word matches any value.
fn matches_tags(record: &Record, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    match query.split_once('=') {
        Some((cat, val)) => record.tags.iter().any(|(c, vals)| {
            contains_ci(c, cat.trim()) && vals.iter().any(|v| contains_ci(v, val.trim()))
        }),
        None => record
            .tags
            .values()
            .flatten()
            .any(|v| contains_ci(v, query)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn rec(id: &str, name: &str, tags: &[(&str, &[&str])]) -> Record {
        let tags = tags
            .iter()
            .map(|(c, vs)| (c.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect();
        Record {
            id: id.into(),
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size: 1,
            tags,
        }
    }

    #[test]
    fn each_kind_matches() {
        let records = vec![
            rec("a", "Concert.mkv", &[("genre", &["rock"])]),
            rec("b", "interview.mp3", &[("genre", &["Jazz"])]),
            rec("c", "notes.txt", &[]),
        ];
        let mut f = FilterState {
            query: "concert".into(),
            ..Default::default()
        };
        assert_eq!(visible_indices(&records, &f), vec![0]);
        f.kind = FilterKind::Tags;
        f.query = "jazz".into();
        assert_eq!(visible_indices(&records, &f), vec![1]);
        f.query = "genre=roc".into();
        assert_eq!(visible_indices(&records, &f), vec![0]);
        f.kind = FilterKind::Untagged;
        assert_eq!(visible_indices(&records, &f), vec![2]);
    }

    #[test]
    fn kind_round_trips() {
        assert_eq!(FilterKind::parse("tags"), FilterKind::Tags);
        assert_eq!(FilterKind::parse("nope"), FilterKind::Filename);
        assert_eq!(FilterKind::Tags.name(), "tags");
        let mut f = FilterState::default();
        f.cycle(1);
        assert_eq!(f.kind, FilterKind::Tags);
        f.clear();
        assert_eq!(f.kind, FilterKind::Filename);
        let empty: BTreeMap<String, Vec<String>> = BTreeMap::new();
        assert_eq!(crate::model::format_tags(&empty), "-");
    }
}
