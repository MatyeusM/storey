//! Tag validation, mutation, queries, and `<target>` resolution.
//!
//! Targets are paths inside the library (view links or store
//! objects, symlinks resolved, else content-hash match) or full ids
//! / unambiguous prefixes.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;

use crate::hash;
use crate::index::{Index, Record};

/// Failure payload for unresolvable targets.
#[derive(Debug, Serialize)]
pub struct TargetError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
}

/// Resolve `arg` to a record position.
pub fn resolve_target(
    root: &Path,
    ix: &Index,
    arg: &str,
) -> std::result::Result<usize, TargetError> {
    if let Some(pos) = match_path(root, ix, arg) {
        return Ok(pos);
    }
    if looks_like_path(arg) {
        return Err(TargetError {
            error: format!("no indexed file at {arg:?}"),
            candidates: None,
        });
    }
    let lowered = arg.to_lowercase();
    match ix.find_prefix(&lowered) {
        Err(_) => {
            let candidates: Vec<String> = ix
                .files
                .iter()
                .filter(|r| r.id.starts_with(&lowered))
                .map(|r| r.id.clone())
                .collect();
            Err(TargetError {
                error: format!("ambiguous id prefix {arg:?}"),
                candidates: Some(candidates),
            })
        }
        Ok(None) => Err(TargetError {
            error: format!("no such file or id {arg:?}"),
            candidates: None,
        }),
        Ok(Some(rec)) => ix
            .files
            .iter()
            .position(|r| r.id == rec.id)
            .ok_or(TargetError {
                error: format!("no such file or id {arg:?}"),
                candidates: None,
            }),
    }
}

fn looks_like_path(arg: &str) -> bool {
    if arg.contains('/') || arg.contains(std::path::MAIN_SEPARATOR) {
        return true;
    }
    std::fs::symlink_metadata(arg).is_ok()
}

fn match_path(root: &Path, ix: &Index, arg: &str) -> Option<usize> {
    let abs = if Path::new(arg).is_absolute() {
        PathBuf::from(arg)
    } else {
        std::env::current_dir().ok()?.join(arg)
    };
    std::fs::symlink_metadata(&abs).ok()?;
    let resolved = std::fs::canonicalize(&abs).ok()?;
    let rel = resolved.strip_prefix(root).ok()?;
    if rel
        .components()
        .next()
        .map(|c| c.as_os_str() == "..")
        .unwrap_or(false)
    {
        return None;
    }
    let rel = slash(rel);
    if let Some(pos) = ix.files.iter().position(|r| r.location() == rel) {
        return Some(pos);
    }
    let (hash, _) = hash::file(&resolved).ok()?;
    ix.files.iter().position(|r| r.id == hash)
}

fn slash(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Distinct categories across the index, sorted.
pub fn categories(files: &[Record]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for rec in files {
        for cat in rec.tags.keys() {
            if seen.insert(cat.clone()) {
                out.push(cat.clone());
            }
        }
    }
    out.sort();
    out
}

/// Replace (`merge` false) or merge (`merge` true) a category.
pub fn apply_values(rec: &mut Record, cat: &str, vals: Vec<String>, merge: bool) {
    if merge {
        let entry = rec.tags.entry(cat.to_string()).or_default();
        for v in vals {
            if !entry.contains(&v) {
                entry.push(v);
            }
        }
    } else {
        rec.tags.insert(cat.to_string(), vals);
    }
}

/// Drop values, a category, or all tags; returns a usage message on
/// invalid input, if any.
pub fn apply_remove(rec: &mut Record, cat: &str, raw: &str) -> Result<(), String> {
    if cat.is_empty() {
        rec.tags.clear();
        return Ok(());
    }
    check_name(cat).map_err(|e| format!("tag remove: {e}"))?;
    if raw.is_empty() {
        rec.tags.remove(cat);
        return Ok(());
    }
    let vals = split_values(raw).map_err(|e| format!("tag remove: {e}"))?;
    match rec.tags.get_mut(cat) {
        None => Ok(()),
        Some(have) => {
            have.retain(|v| !vals.contains(v));
            if have.is_empty() {
                rec.tags.remove(cat);
            }
            Ok(())
        }
    }
}

/// Split comma-separated values: trimmed, deduped; empty entries
/// and bad names are errors.
pub fn split_values(raw: &str) -> Result<Vec<String>, String> {
    let mut vals = Vec::new();
    for part in raw.split(',') {
        let v = part.trim();
        if v.is_empty() {
            return Err(format!("empty value in {raw:?}"));
        }
        check_name(v).map_err(|e| e.to_string())?;
        if !vals.contains(&v.to_string()) {
            vals.push(v.to_string());
        }
    }
    Ok(vals)
}

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Validate tag/category/display strings: non-empty, no
/// `<>:"/\\|?*,` or controls, no trailing dot/space, no reserved
/// names. Commas are banned because they separate values.
pub fn check_name(s: &str) -> Result<(), String> {
    if s.trim().is_empty() {
        return Err("name must not be empty".into());
    }
    for c in s.chars() {
        if c < '\u{20}' || c == '\u{7f}' {
            return Err(format!("name {s:?} contains control characters"));
        }
        if matches!(
            c,
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | ','
        ) {
            return Err(format!("name {s:?} contains illegal character {c:?}"));
        }
    }
    if s.ends_with('.') || s.ends_with(' ') {
        return Err(format!("name {s:?} must not end with a dot or space"));
    }
    let stem = s.split('.').next().unwrap_or(s);
    if RESERVED.contains(&stem.to_uppercase().as_str()) {
        return Err(format!("name {s:?} is reserved"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_names() {
        assert!(check_name("rock").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name("a,b").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("trail.").is_err());
        assert!(check_name("CON").is_err());
        assert!(check_name("con.txt").is_err());
        assert!(check_name("ok-name_2").is_ok());
    }

    #[test]
    fn splits_and_dedupes() {
        assert_eq!(split_values("a, b,a").unwrap(), vec!["a", "b"]);
        assert!(split_values("a,,b").is_err());
        assert!(split_values("a,<>").is_err());
    }

    #[test]
    fn mutates_records() {
        let mut rec = Record {
            id: "x".into(),
            display_name: "x".into(),
            original_rel: "x".into(),
            stored_rel: String::new(),
            size: 1,
            tags: Default::default(),
        };
        apply_values(&mut rec, "g", vec!["rock".into()], false);
        apply_values(&mut rec, "g", vec!["rock".into(), "pop".into()], true);
        assert_eq!(rec.tags["g"], vec!["rock", "pop"]);
        apply_remove(&mut rec, "g", "rock").unwrap();
        assert_eq!(rec.tags["g"], vec!["pop"]);
        apply_remove(&mut rec, "g", "").unwrap();
        assert!(!rec.tags.contains_key("g"));
        apply_remove(&mut rec, "", "").unwrap();
        assert!(rec.tags.is_empty());
    }

    #[test]
    fn resolves_paths_and_ids() {
        let dir = std::env::temp_dir().join(format!("storey-core-tag-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        let mut ix = Index::new();
        ix.files.push(Record {
            id: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            display_name: "a.txt".into(),
            original_rel: "a.txt".into(),
            stored_rel: String::new(),
            size: 5,
            tags: Default::default(),
        });
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        assert_eq!(resolve_target(&dir, &ix, "a.txt").unwrap(), 0);
        assert_eq!(resolve_target(&dir, &ix, "2cf24dba").unwrap(), 0);
        assert!(resolve_target(&dir, &ix, "missing.txt").is_err());
        assert!(resolve_target(&dir, &ix, "zzz").is_err());
        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
