//! Record-level adoption moves (sibling of `super`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::{Result, problem};
use crate::index::{self, Index, Record};
use crate::library::from_slash;
use crate::views;

/// Verified record whose id is already indexed: the same path is
/// the same object (skip), another path is duplicate content (purge
/// the merged file). Both outcomes resolve the record.
pub fn adopt_duplicate(
    sub: &str,
    sub_abs: &Path,
    rec: &Record,
    dup: &Record,
    res: &mut Result,
) -> bool {
    if dup.location() == format!("{sub}/{}", rec.original_rel) {
        return true;
    }
    let loc = sub_abs.join(from_slash(rec.location()));
    if let Err(e) = std::fs::remove_file(&loc) {
        return problem(res, &format!("{sub}/{}: {e}", rec.location()));
    }
    res.purged.push(rec.id.clone());
    true
}

/// Move a verified stored object from the sub-store into the parent
/// `.files/`, resolving basename collisions with the stem suffix
/// rule, and record it in `ix`.
pub fn adopt_stored(
    root: &Path,
    sub: &str,
    sub_abs: &Path,
    rec: &Record,
    size: i64,
    ix: &mut Index,
    res: &mut Result,
) -> bool {
    let taken = stored_names(ix);
    let name = views::store_name(&rec.display_name, &taken, &rec.id);
    let dst_rel = format!("{}/{}", index::STORE_DIR, name);
    let sub_loc = rec.location().to_string();
    if let Err(e) = std::fs::create_dir_all(root.join(index::STORE_DIR)) {
        return problem(res, &format!("{sub_loc}: {e}"));
    }
    if let Err(e) = std::fs::rename(
        sub_abs.join(from_slash(&sub_loc)),
        root.join(from_slash(&dst_rel)),
    ) {
        return problem(res, &format!("{sub_loc}: {e}"));
    }
    ix.files.push(Record {
        id: rec.id.clone(),
        display_name: rec.display_name.clone(),
        original_rel: format!("{sub}/{}", rec.original_rel),
        stored_rel: dst_rel,
        size,
        tags: copy_tags(&rec.tags),
    });
    res.adopted.push(rec.id.clone());
    true
}

/// Record a verified in-place file with its tags, rebasing its path
/// into parent space. The file stays where it is.
pub fn adopt_in_place(sub: &str, rec: &Record, size: i64, ix: &mut Index, res: &mut Result) {
    ix.files.push(Record {
        id: rec.id.clone(),
        display_name: rec.display_name.clone(),
        original_rel: format!("{sub}/{}", rec.original_rel),
        stored_rel: String::new(),
        size,
        tags: copy_tags(&rec.tags),
    });
    res.adopted.push(rec.id.clone());
}

fn stored_names(ix: &Index) -> HashMap<String, String> {
    let mut taken = HashMap::new();
    for rec in &ix.files {
        if rec.stored() {
            let base = rec.stored_rel.rsplit('/').next().unwrap_or(&rec.stored_rel);
            taken.insert(base.to_string(), rec.id.clone());
        }
    }
    taken
}

fn copy_tags(tags: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    tags.iter().map(|(c, v)| (c.clone(), v.clone())).collect()
}
