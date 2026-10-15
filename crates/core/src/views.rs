//! Pure view mapping: which store object links from which view
//! path. Shared by view (build), rm and restore (derive).
//! Deterministic in record order, so rebuilds are stable without
//! storing the mapping.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::index::Record;

/// View folder for objects missing a value at a level.
pub const UNSORTED: &str = "Unsorted";

/// One store object linked from one view path (slash-separated,
/// relative to root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub view_path: String,
    pub id: String,
}

/// View links for records under key order `by`. A missing value at
/// a level maps to `Unsorted/`; multiple values expand to the
/// cartesian product across levels. Collisions (same path, or paths
/// differing only by case) get a deterministic `__2`, `__3`, ...
/// suffix on the link basename, in record order.
pub fn plan(records: &[Record], by: &[String]) -> Vec<Link> {
    let mut links = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for rec in records {
        for p in expand(rec, by) {
            let p = disambiguate(&p, &used);
            used.insert(p.to_lowercase());
            links.push(Link {
                view_path: p,
                id: rec.id.clone(),
            });
        }
    }
    links
}

fn expand(rec: &Record, by: &[String]) -> Vec<String> {
    // Missing every level: one flat Unsorted, not one per level.
    if !by.is_empty() && missing_all(rec, by) {
        return vec![join(UNSORTED, &rec.display_name)];
    }
    let mut dirs = vec![String::new()];
    for cat in by {
        let vals: Vec<&str> = match rec.tags.get(cat) {
            Some(v) if !v.is_empty() => v.iter().map(|s| s.as_str()).collect(),
            _ => vec![UNSORTED],
        };
        let mut next = Vec::with_capacity(dirs.len() * vals.len());
        for d in &dirs {
            for v in &vals {
                next.push(join(d, v));
            }
        }
        dirs = next;
    }
    dirs.iter().map(|d| join(d, &rec.display_name)).collect()
}

fn missing_all(rec: &Record, by: &[String]) -> bool {
    by.iter()
        .all(|cat| rec.tags.get(cat).map(|v| v.is_empty()).unwrap_or(true))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

fn disambiguate(path: &str, used: &HashSet<String>) -> String {
    if !used.contains(&path.to_lowercase()) {
        return path.to_string();
    }
    let (dir, base) = split(path);
    let ext = extension(base);
    let stem = base.strip_suffix(ext).unwrap_or(base);
    let mut i = 2;
    loop {
        let candidate = join(dir, &format!("{stem}__{i}{ext}"));
        if !used.contains(&candidate.to_lowercase()) {
            return candidate;
        }
        i += 1;
    }
}

fn split(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => ("", path),
    }
}

/// Extension from the final dot in the basename, like Go's
/// `filepath.Ext` (leading dots count, `a.` gives `.`).
fn extension(base: &str) -> &str {
    match base.rfind('.') {
        Some(i) => &base[i..],
        None => "",
    }
}

/// Derived view link paths as a set (empty when no view was ever
/// materialized, so plain filenames are never misclassified).
pub fn plan_set(records: &[Record], by: &[String]) -> HashSet<String> {
    if by.is_empty() {
        return HashSet::new();
    }
    plan(
        &records
            .iter()
            .filter(|r| r.stored())
            .cloned()
            .collect::<Vec<_>>(),
        by,
    )
    .into_iter()
    .map(|l| l.view_path)
    .collect()
}

/// Store object name for display: display itself, or `stem_N.ext`
/// while the name is taken by different content. `taken` maps
/// *lowercase* store basename to id, because case-insensitive
/// filesystems (macOS, Windows, exFAT) would let a case-only
/// difference overwrite the first object.
pub fn store_name(display: &str, taken: &HashMap<String, String>, id: &str) -> String {
    if taken
        .get(&display.to_lowercase())
        .map(|o| o == id)
        .unwrap_or(true)
    {
        return display.to_string();
    }
    let ext = extension(display);
    let stem = display.strip_suffix(ext).unwrap_or(display);
    let mut i = 1;
    loop {
        let candidate = format!("{stem}_{i}{ext}");
        if taken
            .get(&candidate.to_lowercase())
            .map(|o| o == id)
            .unwrap_or(true)
        {
            return candidate;
        }
        i += 1;
    }
}

/// Create `link` pointing at the store object: symlink preferred,
/// hard link otherwise. No copy fallback — storey never copies file
/// content. Both failing is an error; callers log and carry on.
pub fn create_link(store_abs: &Path, link_abs: &Path) -> Result<&'static str, std::io::Error> {
    let rel = relative_to(store_abs, link_abs.parent().unwrap_or(Path::new("")));
    let target = rel.unwrap_or_else(|_| store_abs.to_path_buf());
    match symlink(&target, link_abs) {
        Ok(()) => Ok("symlink"),
        Err(_) => match std::fs::hard_link(store_abs, link_abs) {
            Ok(()) => Ok("hardlink"),
            Err(e) => Err(e),
        },
    }
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> Result<(), std::io::Error> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> Result<(), std::io::Error> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(any(unix, windows)))]
fn symlink(target: &Path, link: &Path) -> Result<(), std::io::Error> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("symlinks unsupported for {}", target.display()),
    ))
}

/// Lexical relative path from `base` to `path` (no fs access).
fn relative_to(path: &Path, base: &Path) -> Result<std::path::PathBuf, ()> {
    use std::path::Component;
    let mut p = path.components().peekable();
    let mut b = base.components().peekable();
    loop {
        match (p.peek(), b.peek()) {
            (Some(a), Some(x)) if a == x => {
                p.next();
                b.next();
            }
            _ => break,
        }
    }
    if b.peek().is_none() && p.peek().is_none() {
        return Ok(Path::new("").to_path_buf());
    }
    let mut out = std::path::PathBuf::new();
    for c in b {
        match c {
            Component::Normal(_) => out.push(".."),
            Component::ParentDir => return Err(()),
            _ => {}
        }
    }
    for c in p {
        match c {
            Component::Normal(s) => out.push(s),
            _ => return Err(()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn rec(id: &str, name: &str, tags: &[(&str, &[&str])]) -> Record {
        Record {
            id: id.into(),
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size: 1,
            tags: tags
                .iter()
                .map(|(c, vs)| (c.to_string(), vs.iter().map(|v| v.to_string()).collect()))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn plans_cartesian_and_unsorted() {
        let recs = vec![
            rec("a", "f.mkv", &[("genre", &["rock", "pop"])]),
            rec("b", "g.mkv", &[]),
        ];
        let links = plan(&recs, &["genre".to_string()]);
        let paths: Vec<_> = links.iter().map(|l| l.view_path.as_str()).collect();
        assert_eq!(paths, vec!["rock/f.mkv", "pop/f.mkv", "Unsorted/g.mkv"]);
    }

    #[test]
    fn disambiguates_case_collisions() {
        let recs = vec![rec("a", "F.mkv", &[]), rec("b", "f.mkv", &[])];
        let links = plan(&recs, &["genre".to_string()]);
        let paths: Vec<_> = links.iter().map(|l| l.view_path.as_str()).collect();
        assert_eq!(paths, vec!["Unsorted/F.mkv", "Unsorted/f__2.mkv"]);
    }

    #[test]
    fn store_names_collide_by_content() {
        let taken: HashMap<String, String> = [("f.mkv".to_string(), "other".to_string())]
            .into_iter()
            .collect();
        assert_eq!(store_name("f.mkv", &taken, "other"), "f.mkv");
        assert_eq!(store_name("f.mkv", &taken, "mine"), "f_1.mkv");
        assert_eq!(store_name("new.mkv", &taken, "mine"), "new.mkv");
    }

    #[test]
    fn store_names_collide_case_insensitively() {
        let taken: HashMap<String, String> = [("f.mkv".to_string(), "other".to_string())]
            .into_iter()
            .collect();
        assert_eq!(store_name("F.MKV", &taken, "other"), "F.MKV");
        assert_eq!(store_name("F.MKV", &taken, "mine"), "F_1.MKV");
    }

    #[test]
    fn links_prefer_symlinks() {
        let dir = std::env::temp_dir().join(format!("storey-core-view-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("v")).unwrap();
        let store = dir.join(".files").join("f");
        std::fs::create_dir_all(dir.join(".files")).unwrap();
        std::fs::write(&store, "x").unwrap();
        let link = dir.join("v").join("f");
        assert_eq!(create_link(&store, &link).unwrap(), "symlink");
        assert_eq!(std::fs::read(&link).unwrap(), b"x");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
