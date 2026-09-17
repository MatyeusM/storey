//! Pure record helpers: category counts, id lookup, value splitting.
//!
//! No CLI calls here so unit tests never need a binary.

use storey_shared::model::Record;

/// Distinct categories across `records` with usage counts, sorted by name.
pub fn categories_with_counts(records: &[Record]) -> Vec<(String, usize)> {
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for record in records {
        for cat in record.tags.keys() {
            *counts.entry(cat.as_str()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(cat, n)| (cat.to_string(), n))
        .collect()
}

/// Find a record by full id.
pub fn find_by_id<'a>(records: &'a [Record], id: &str) -> Option<&'a Record> {
    records.iter().find(|r| r.id == id)
}

/// Split a values buffer on `,`, trimming whitespace and dropping empties.
pub fn split_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

/// First category starting with `prefix` (case-insensitive), for Tab completion.
pub fn complete_category(categories: &[(String, usize)], prefix: &str) -> Option<String> {
    let lower = prefix.to_ascii_lowercase();
    categories
        .iter()
        .map(|(cat, _)| cat)
        .find(|cat| cat.to_ascii_lowercase().starts_with(&lower))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn record(id: &str, tags: &[(&str, &[&str])]) -> Record {
        let mut map = BTreeMap::new();
        for (cat, vals) in tags {
            map.insert(
                (*cat).to_string(),
                vals.iter().map(|v| (*v).to_string()).collect(),
            );
        }
        Record {
            id: id.into(),
            display_name: format!("{id}.mkv"),
            original_rel: format!("{id}.mkv"),
            stored_rel: String::new(),
            size: 1,
            tags: map,
        }
    }

    #[test]
    fn counts_sort_and_tally() {
        let recs = vec![
            record("a", &[("actor", &["X"]), ("date", &["2020"])]),
            record("b", &[("actor", &["Y"])]),
        ];
        assert_eq!(
            categories_with_counts(&recs),
            vec![("actor".into(), 2), ("date".into(), 1)]
        );
    }

    #[test]
    fn lookup_and_split() {
        let recs = vec![record("ab", &[])];
        assert!(find_by_id(&recs, "ab").is_some());
        assert!(find_by_id(&recs, "zz").is_none());
        assert_eq!(split_values("a, b,, c "), vec!["a", "b", "c"]);
        assert!(split_values(" , ").is_empty());
    }

    #[test]
    fn completion_is_prefix_case_insensitive() {
        let cats = vec![("actor".to_string(), 2), ("date".to_string(), 1)];
        assert_eq!(complete_category(&cats, "ac"), Some("actor".into()));
        assert_eq!(complete_category(&cats, "D"), Some("date".into()));
        assert_eq!(complete_category(&cats, "z"), None);
    }
}
