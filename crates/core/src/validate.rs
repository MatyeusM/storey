//! Validation of data read back from `.index.json`.
//!
//! The index is data, not code — but every path in it is later used as a
//! filesystem operation. Nothing may escape the library root: every indexed
//! path is either at the root or in a subfolder. We enforce that here, at the
//! single point where untrusted index bytes enter the process (`Index::load`),
//! so all callers (CLI, GUI flows, nested adoption) inherit the guarantee.

use std::path::Component;

use anyhow::{Result, bail};

use crate::index::{Index, STORE_DIR};
use crate::tags::check_name;

fn has_control(s: &str) -> bool {
    s.chars().any(char::is_control)
}

/// Validate a slash-relative path that must stay inside the library root.
/// Rejects absolute paths, `.`/`..` components, backslashes (a path
/// separator on Windows), empty segments and control characters.
pub fn rel(path: &str) -> std::result::Result<(), String> {
    let bad = |why: &str| format!("{path:?}: {why}");
    if path.is_empty() {
        return Err(bad("empty path"));
    }
    if path.contains('\\') {
        return Err(bad("backslash is a path separator on Windows"));
    }
    if has_control(path) {
        return Err(bad("control character"));
    }
    if let Some(first) = path.split('/').next()
        && first.len() == 2
        && first.ends_with(':')
        && first
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return Err(bad("Windows drive prefix"));
    }
    for component in std::path::Path::new(path).components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(bad("must be relative with no '.' or '..'"));
        }
    }
    if path
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(bad("must be relative with no '.' or '..'"));
    }
    Ok(())
}

/// Validate a single path component (display name, store basename, tag).
pub fn basename(name: &str) -> std::result::Result<(), String> {
    let bad = |why: &str| format!("{name:?}: {why}");
    if name.is_empty() {
        return Err(bad("empty name"));
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(bad("must be a single path component"));
    }
    if has_control(name) {
        return Err(bad("control character"));
    }
    Ok(())
}

/// Validate every datum in a loaded index. Tags go through `check_name`
/// (the same charset enforced at tag entry), paths through `rel`/`basename`.
pub fn index(ix: &Index) -> Result<()> {
    for cat in &ix.view.by {
        check_name(cat).map_err(|e| anyhow::anyhow!("view.by {cat:?}: {e}"))?;
    }
    for rec in &ix.files {
        basename(&rec.display_name)
            .map_err(|e| anyhow::anyhow!("record {}: displayName {e}", rec.id))?;
        rel(&rec.original_rel)
            .map_err(|e| anyhow::anyhow!("record {}: originalRel {e}", rec.id))?;
        if rec.size < 0 {
            bail!("record {}: negative size {}", rec.id, rec.size);
        }
        if rec.stored() {
            let prefix = format!("{STORE_DIR}/");
            if !rec.stored_rel.starts_with(&prefix) {
                bail!("record {}: storedRel must start with {prefix:?}", rec.id);
            }
            rel(&rec.stored_rel)
                .map_err(|e| anyhow::anyhow!("record {}: storedRel {e}", rec.id))?;
            let base = &rec.stored_rel[prefix.len()..];
            basename(base).map_err(|e| anyhow::anyhow!("record {}: storedRel {e}", rec.id))?;
        }
        for dup in &rec.duplicates {
            rel(dup).map_err(|e| anyhow::anyhow!("record {}: duplicates {e}", rec.id))?;
        }
        for (cat, values) in &rec.tags {
            check_name(cat)
                .map_err(|e| anyhow::anyhow!("record {}: category {cat:?}: {e}", rec.id))?;
            for value in values {
                check_name(value)
                    .map_err(|e| anyhow::anyhow!("record {}: {cat}={value:?}: {e}", rec.id))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{Header, View};
    use std::collections::BTreeMap;

    fn rec(id: &str, orig: &str) -> crate::index::Record {
        crate::index::Record {
            id: id.into(),
            display_name: "a.txt".into(),
            original_rel: orig.into(),
            stored_rel: String::new(),
            size: 1,
            duplicates: vec![],
            tags: BTreeMap::new(),
        }
    }

    fn ix(orig: &str) -> Index {
        Index {
            storey: Header {
                schema: crate::index::SCHEMA,
                algorithm: crate::index::ALGORITHM.into(),
            },
            view: View { by: vec![] },
            files: vec![rec("id", orig)],
        }
    }

    #[test]
    fn accepts_in_library_paths() {
        assert!(index(&ix("a.txt")).is_ok());
        assert!(index(&ix("films/2020/a.txt")).is_ok());
        assert!(rel("a/b/c.txt").is_ok());
    }

    #[test]
    fn rejects_escaping_paths() {
        for bad in [
            "../a.txt",
            "a/../../b",
            "/etc/passwd",
            "a/./b",
            "a//b",
            "a\\b",
        ] {
            assert!(rel(bad).is_err(), "{bad} should be rejected");
            assert!(
                index(&ix(bad)).is_err(),
                "{bad} should be rejected in index"
            );
        }
        assert!(index(&ix("C:/Windows/x")).is_err());
    }

    #[test]
    fn rejects_bad_names_and_tags() {
        assert!(basename("../x").is_err());
        assert!(basename("a/b").is_err());
        let mut doc = ix("a.txt");
        doc.files[0].display_name = "../evil".into();
        assert!(index(&doc).is_err());
        let mut doc = ix("a.txt");
        doc.files[0]
            .tags
            .insert("g".into(), vec!["../../evil".into()]);
        assert!(index(&doc).is_err());
        let mut doc = ix("a.txt");
        doc.files[0].duplicates = vec!["../../evil".into()];
        assert!(index(&doc).is_err());
    }
}
