//! Duplicate-file recreation for `restore --duplicates`.

use std::path::Path;

use crate::index::Index;
use crate::library;
use crate::restore::RestoreOutcome;

pub fn recreate_duplicates(root: &Path, ix: &Index) -> RestoreOutcome {
    let mut out = RestoreOutcome::default();
    for rec in &ix.files {
        if rec.duplicates.is_empty() {
            continue;
        }
        let src = root.join(library::from_slash(rec.location()));
        for dup in &rec.duplicates {
            if out.duplicate_paths.contains(dup) {
                continue;
            }
            let dst = root.join(library::from_slash(dup));
            if let Some(parent) = dst.parent()
                && let Err(e) = std::fs::create_dir_all(parent)
            {
                out.failures.push(format!("{dup}: {e}"));
                continue;
            }
            if std::fs::symlink_metadata(&dst).is_ok() {
                continue; // already present; never overwrite
            }
            if std::fs::hard_link(&src, &dst).is_ok()
                || crate::views::create_link(&src, &dst).is_ok()
            {
                out.duplicates += 1;
                out.duplicate_paths.push(dup.clone());
            } else {
                out.failures.push(format!("{dup}: cannot recreate"));
            }
        }
    }
    out
}

/// Paths that `restore --duplicates` would recreate (dry-run preview).
pub fn planned_duplicates(ix: &Index) -> Vec<String> {
    let mut paths: Vec<String> = ix
        .files
        .iter()
        .flat_map(|rec| rec.duplicates.iter().cloned())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{Record, STORE_DIR};
    use std::collections::BTreeMap;

    fn fixture(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("storey-core-restore-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(STORE_DIR)).unwrap();
        dir
    }

    #[test]
    fn duplicates_are_recreated_from_the_canonical_object() {
        let dir = fixture("dups");
        std::fs::write(dir.join("movie.mkv"), "content").unwrap();
        let mut ix = Index::new();
        ix.files.push(Record {
            id: "abc".into(),
            display_name: "movie.mkv".into(),
            original_rel: "movie.mkv".into(),
            stored_rel: String::new(),
            duplicates: vec!["copies/movie.mkv".into(), "movie_1.mkv".into()],
            size: 7,
            tags: BTreeMap::new(),
        });
        let outcome = recreate_duplicates(&dir, &ix);
        assert_eq!(outcome.duplicates, 2);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(
            std::fs::read(dir.join("copies/movie.mkv")).unwrap(),
            b"content"
        );
        assert_eq!(std::fs::read(dir.join("movie_1.mkv")).unwrap(), b"content");
        // Idempotent: a second run leaves them alone.
        assert_eq!(recreate_duplicates(&dir, &ix).duplicates, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
