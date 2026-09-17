//! Breadcrumb helpers for repo-mode tag navigation.
//!
//! Pure functions so `app`, `topbar`, and tests share one geometry:
//! chips are `[tag -]` joined by ` >> `, plus a trailing `[Add +]`
//! and — when the view is empty — a `[Restore]` chip after it.

use std::path::{Path, PathBuf};

/// Trailing add chip label.
pub const ADD_LABEL: &str = "[Add +]";
/// Restore chip label, shown only with an empty view.
pub const RESTORE_LABEL: &str = "[Restore]";
/// Separator between chips.
pub const SEPARATOR: &str = " >> ";

/// Last path component (`Audio` for `.../Elements/Audio`).
pub fn basename_of(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// Label for one active view tag.
pub fn chip_label(tag: &str) -> String {
    format!("[{tag} -]")
}

/// All chip labels: one per view tag, `[Add +]`, and `[Restore]`
/// when the view is empty (nothing materialized to undo otherwise).
pub fn chip_labels(view: &[String]) -> Vec<String> {
    let mut out: Vec<String> = view.iter().map(|t| chip_label(t)).collect();
    out.push(ADD_LABEL.to_string());
    if view.is_empty() {
        out.push(RESTORE_LABEL.to_string());
    }
    out
}

/// Categories not already in the view, keeping CLI order.
pub fn available(categories: &[String], view: &[String]) -> Vec<String> {
    categories
        .iter()
        .filter(|c| !view.contains(c))
        .cloned()
        .collect()
}

/// Terminal column ranges for each chip (including `[Add +]`).
///
/// `base_col` is the first content column inside the block border
/// (normally 1). Ranges are `[start, end)` in terminal columns.
pub fn chip_ranges(base_col: u16, basename: &str, view: &[String]) -> Vec<(u16, u16)> {
    let mut col = base_col + chars_len(basename) as u16 + 1;
    let mut out = Vec::new();
    for label in chip_labels(view) {
        let w = chars_len(&label) as u16;
        out.push((col, col + w));
        col += w + chars_len(SEPARATOR) as u16;
    }
    out
}

/// Index of the chip under `col`, if any.
pub fn chip_at(ranges: &[(u16, u16)], col: u16) -> Option<usize> {
    ranges.iter().position(|(s, e)| col >= *s && col < *e)
}

/// True when `idx` points at the trailing `[Add +]` chip.
pub fn is_add_chip(view_len: usize, idx: usize) -> bool {
    idx == view_len
}

/// True when `idx` points at the `[Restore]` chip (empty view only).
pub fn is_restore_chip(view: &[String], idx: usize) -> bool {
    view.is_empty() && idx == view.len() + 1
}

fn chars_len(s: &str) -> usize {
    s.chars().count()
}

/// Root prefix helper for tests.
pub fn _path(p: &str) -> PathBuf {
    Path::new(p).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_keeps_last_component() {
        assert_eq!(
            basename_of(&_path("/run/media/matth/Elements/Audio")),
            "Audio"
        );
    }

    #[test]
    fn available_excludes_viewed() {
        let cats = vec!["a".into(), "b".into(), "c".into()];
        let view = vec!["b".into()];
        assert_eq!(
            available(&cats, &view),
            vec!["a".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn empty_view_appends_restore_chip() {
        let view: Vec<String> = Vec::new();
        let labels = chip_labels(&view);
        assert_eq!(
            labels,
            vec![ADD_LABEL.to_string(), RESTORE_LABEL.to_string()]
        );
        assert!(is_restore_chip(&view, 1));
        assert!(!is_restore_chip(&view, 0));
        let ranges = chip_ranges(1, "Audio", &view);
        assert_eq!(ranges.len(), 2);
        assert_eq!(chip_at(&ranges, ranges[1].0), Some(1));
    }

    #[test]
    fn ranges_hit_each_chip() {
        let view = vec!["T1".into(), "T2".into()];
        let ranges = chip_ranges(1, "Audio", &view);
        assert_eq!(ranges.len(), 3);
        assert_eq!(chip_at(&ranges, ranges[0].0), Some(0));
        assert_eq!(chip_at(&ranges, ranges[2].0), Some(2));
        assert!(is_add_chip(view.len(), 2));
        assert!(!is_add_chip(view.len(), 0));
    }
}
