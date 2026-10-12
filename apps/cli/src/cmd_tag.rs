//! `tag` wiring: get reads, set/add/remove write under a lock.
//!
//! Flag parsing mirrors the reference CLI exactly (including its
//! usage messages); only argv plumbing lives here.

use crate::{EXIT_FAILURE, EXIT_OK, emit, fail, usage_err};
use storey_core::index::Index;
use storey_core::tags;

/// Split `--category`/`--values` (both `=` and space forms) from
/// positionals. Unknown `--` flags are usage errors.
fn parse_cat_vals(args: &[String]) -> Result<(String, String, Vec<String>), String> {
    let mut cat = String::new();
    let mut raw = String::new();
    let mut pos = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--category" && i + 1 < args.len() {
            i += 1;
            cat = args[i].clone();
        } else if let Some(v) = a.strip_prefix("--category=") {
            cat = v.to_string();
        } else if a == "--values" && i + 1 < args.len() {
            i += 1;
            raw = args[i].clone();
        } else if let Some(v) = a.strip_prefix("--values=") {
            raw = v.to_string();
        } else if a.starts_with("--") {
            return Err(format!("tag: unknown flag {a:?}"));
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Ok((cat, raw, pos))
}

/// Resolve a target: emit the failure payload and print, or return
/// the record position.
fn resolve(
    root: &std::path::Path,
    ix: &Index,
    arg: &str,
    command: &str,
    as_json: bool,
) -> Result<usize, ()> {
    match tags::resolve_target(root, ix, arg) {
        Ok(pos) => Ok(pos),
        Err(e) => {
            emit(command, as_json, &e);
            match e.candidates {
                Some(c) => fail(&format!("ambiguous id prefix {arg:?}: {}", c.join(", "))),
                None => fail(&e.error),
            };
            Err(())
        }
    }
}

/// `tag get`: categories, file tags, or values.
pub fn cmd_get(args: &[String], as_json: bool) -> i32 {
    let (cat, raw, pos) = match parse_cat_vals(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    if !raw.is_empty() {
        return usage_err("tag get: --values is for set/add/remove, not get");
    }
    if pos.len() > 1 {
        return usage_err("tag get takes at most one <target>");
    }
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("{e}")),
    };
    let root = match storey_core::library::find_root(&cwd) {
        Ok(root) => root,
        Err(e) => return usage_err(&format!("{e}")),
    };
    let ix = match Index::load(&root) {
        Ok(ix) => ix,
        Err(e) => return fail(&format!("{e}")),
    };
    if pos.is_empty() {
        if !cat.is_empty() {
            return usage_err("tag get: --category needs a <target>");
        }
        let cats = tags::categories(&ix.files);
        emit("tag", as_json, &cats);
        if !as_json {
            for c in &cats {
                println!("{c}");
            }
        }
        return EXIT_OK;
    }
    if !cat.is_empty()
        && let Err(e) = tags::check_name(&cat)
    {
        return usage_err(&format!("tag get: {e}"));
    }
    emit_for_target(&root, &ix, &pos, &cat, as_json)
}

/// Resolve the target and emit its tags or values.
fn emit_for_target(
    root: &std::path::Path,
    ix: &Index,
    pos: &[String],
    cat: &str,
    as_json: bool,
) -> i32 {
    let idx = match resolve(root, ix, &pos[0], "tag", as_json) {
        Ok(idx) => idx,
        Err(()) => return EXIT_FAILURE,
    };
    let rec = &ix.files[idx];
    if cat.is_empty() {
        emit("tag", as_json, &rec.tags);
        if !as_json {
            println!("{}", storey_core::ops::format_tags(&rec.tags));
        }
        return EXIT_OK;
    }
    let vals = rec.tags.get(cat).cloned().unwrap_or_default();
    emit("tag", as_json, &vals);
    if !as_json {
        for v in &vals {
            println!("{v}");
        }
    }
    EXIT_OK
}

/// `tag set` (replace) and `tag add` (merge).
pub fn cmd_write(args: &[String], as_json: bool, merge: bool) -> i32 {
    let (cat, raw, pos) = match parse_cat_vals(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    if pos.len() != 1 {
        return usage_err("tag set/add takes exactly one <target>");
    }
    if cat.is_empty() || raw.is_empty() {
        return usage_err("tag set/add: --category and --values are both required");
    }
    if let Err(e) = tags::check_name(&cat) {
        return usage_err(&format!("tag: {e}"));
    }
    let vals = match tags::split_values(&raw) {
        Ok(vals) => vals,
        Err(e) => return usage_err(&format!("tag: {e}")),
    };
    locked_write(as_json, &cat, |root, ix| {
        let idx = resolve(root, ix, &pos[0], "tag", as_json)?;
        let rec = &mut ix.files[idx];
        tags::apply_values(rec, &cat, vals, merge);
        Ok(idx)
    })
}

/// `tag remove`: values, a category, or all tags by arity.
pub fn cmd_remove(args: &[String], as_json: bool) -> i32 {
    let (cat, raw, pos) = match parse_cat_vals(args) {
        Ok(v) => v,
        Err(msg) => return usage_err(&msg),
    };
    if pos.len() != 1 {
        return usage_err("tag remove takes exactly one <target>");
    }
    if !raw.is_empty() && cat.is_empty() {
        return usage_err("tag remove: --values needs --category");
    }
    locked_write(as_json, &cat, |root, ix| {
        let idx = resolve(root, ix, &pos[0], "tag", as_json)?;
        tags::apply_remove(&mut ix.files[idx], &cat, &raw).map_err(WriteErr::Usage)?;
        Ok(idx)
    })
}

enum WriteErr {
    Usage(String),
    Abort,
}

impl From<()> for WriteErr {
    fn from(_: ()) -> Self {
        WriteErr::Abort
    }
}

/// Lock, load, mutate via `op`, save, emit the record, print.
fn locked_write(
    as_json: bool,
    cat: &str,
    op: impl FnOnce(&std::path::Path, &mut Index) -> Result<usize, WriteErr>,
) -> i32 {
    let mut opened = match crate::open_locked() {
        Ok(o) => o,
        Err(code) => return code,
    };
    let root = opened.root.clone();
    let idx = match op(&root, &mut opened.ix) {
        Ok(idx) => idx,
        Err(WriteErr::Usage(msg)) => return usage_err(&msg),
        Err(WriteErr::Abort) => return EXIT_FAILURE,
    };
    if let Err(e) = opened.ix.save(&root) {
        return fail(&format!("{e}"));
    }
    let rec = &opened.ix.files[idx];
    emit("tag", as_json, rec);
    if !as_json {
        if cat.is_empty() {
            println!("removed all tags from {}", rec.id);
        } else {
            println!(
                "{}: {}=[{}]",
                rec.id,
                cat,
                rec.tags.get(cat).map(|v| v.join(" ")).unwrap_or_default()
            );
        }
    }
    EXIT_OK
}

/// Dispatch `tag` subcommands: get, set, add, remove.
pub fn cmd_tag(args: &[String], as_json: bool) -> i32 {
    if args.is_empty() {
        return tag_usage();
    }
    match args[0].as_str() {
        "get" => cmd_get(&args[1..], as_json),
        "set" => cmd_write(&args[1..], as_json, false),
        "add" => cmd_write(&args[1..], as_json, true),
        "remove" => cmd_remove(&args[1..], as_json),
        verb => usage_err(&format!("tag: unknown subcommand {verb:?}")),
    }
}

/// Usage error text for `tag` with no subcommand.
pub fn tag_usage() -> i32 {
    usage_err(
        "tag: usage: tag get [target] [--category=C] | tag set|add <target> --category=C --values=a,b | tag remove <target> [--category=C] [--values=a]",
    )
}
