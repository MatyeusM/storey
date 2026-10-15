//! Record-level adoption moves (sibling of `super`).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::{Result, StoredCtx};
use crate::index::{self, Index, Record};
use crate::library::from_slash;
use crate::views;

/// Duplicate-resolution context (keeps arity small).
pub struct DupCtx<'a> {
    pub root: &'a Path,
    pub sub: &'a str,
    pub sub_abs: &'a Path,
    pub rec: &'a Record,
    pub dup: &'a Record,
    pub res: &'a mut Result,
}

/// Verified record whose id is already indexed: the same path is
/// the same object (skip), another path is duplicate content. The
/// merged file is deleted only when the parent's copy is really on
/// disk (never delete the last copy on doubt).
pub fn adopt_duplicate(ctx: DupCtx) -> bool {
    let DupCtx {
        root,
        sub,
        sub_abs,
        rec,
        dup,
        res,
    } = ctx;
    if dup.location() == format!("{sub}/{}", rec.original_rel) {
        return true;
    }
    if !crate::intake::on_disk(root, dup.location()) {
        return problem(
            res,
            &format!(
                "{sub}/{}: duplicate of {} which is missing; file kept",
                rec.location(),
                dup.location()
            ),
        );
    }
    let loc = sub_abs.join(from_slash(rec.location()));
    if let Err(e) = std::fs::remove_file(&loc) {
        return problem(res, &format!("{sub}/{}: {e}", rec.location()));
    }
    res.purged.push(rec.id.clone());
    true
}

/// Move files left in a nested store into the parent store. The sub
/// directory is disposable, its bytes are not: nothing here is
/// deleted. Unreferenced arrivals are reported by `reconcile_store`
/// (which repoints on a hash match) like any other store residue.
pub(crate) fn move_store_leftovers(
    root: &Path,
    sub_abs: &Path,
    ix: &Index,
    res: &mut Result,
) -> bool {
    let Ok(entries) = std::fs::read_dir(sub_abs.join(index::STORE_DIR)) else {
        return true;
    };
    let dst_dir = root.join(index::STORE_DIR);
    if let Err(e) = std::fs::create_dir_all(&dst_dir) {
        return problem(res, &format!("{}/{}: {e}", index::STORE_DIR, "leftover"));
    }
    let mut taken = stored_names(ix);
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            return problem(
                res,
                &format!(
                    "{}/{name}: not a regular file, left in place",
                    index::STORE_DIR
                ),
            );
        }
        let dst_name = views::store_name(&name, &taken, "");
        if let Err(e) = std::fs::rename(entry.path(), dst_dir.join(&dst_name)) {
            return problem(res, &format!("{}/{name}: {e}", index::STORE_DIR));
        }
        taken.insert(dst_name.to_lowercase(), String::new());
    }
    true
}

/// Record a verification failure against the merge result.
pub(crate) fn problem(res: &mut Result, msg: &str) -> bool {
    res.problems.push(msg.to_string());
    false
}

/// Move a verified stored object from the sub-store into the parent
/// `.files/`, resolving basename collisions with the stem suffix
/// rule, and record it in `ix`.
pub(crate) fn adopt_stored(ctx: StoredCtx) -> bool {
    let StoredCtx {
        root,
        sub,
        sub_abs,
        rec,
        size,
        ix,
        res,
    } = ctx;
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
            taken.insert(base.to_lowercase(), rec.id.clone());
        }
    }
    taken
}

fn copy_tags(tags: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    tags.iter().map(|(c, v)| (c.clone(), v.clone())).collect()
}
