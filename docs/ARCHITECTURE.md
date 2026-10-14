# Architecture (reimplementation plan)

Reference: `.reference/` (Java+ Swing `Clanker Sort`). Do not port it
line-for-line — reimplement the concept in Rust with a thin Qt UI
over linked crates. This doc records
decisions; items below are **locked** once implemented.

## Concept

A managed library folder contains:

1. **Content store** — originals keyed by content hash (SHA-256).
   Locked: `.files/` (reference parity), created by `view`, not `init`.
2. **Index** — tags, hashes, view state. Locked: `.index.json`, JSON,
   namespaced under a `storey` header object with schema version + hash
   algorithm (see `CLI-CONTRACT.md`).
3. **Views** — one folder per value of the chosen view key order,
   containing links back into the store (symlink preferred, then hard
   link — never a copy). Missing value at a level → `Unsorted/`;
   multi-value → linked under each (cartesian product).
4. **Nested libraries** — a subdirectory with its own `.index.json` is
   verified and folded into the parent by `init`/`update` (metadata
   preserved, nested index removed on success).

## Process architecture

- `storey` (Rust CLI) owns **all** logic: scan/hash, index read/write,
  nested-library adoption, view materialization, link strategy,
  filename validation, OS open helpers. Hashing runs on a worker pool
  (NumCPU-1, one core left for UI/OS) and reports progress on stderr
  (human text, or JSONL under `--json`) for the UIs to consume.
- `storey-gui` (Rust/cxx-qt) is a thin Qt shell over the `ui`
  crate: it links `core` directly (no subprocess) and never touches
  the store or index except through `core` flows. UI state, plugin
  queries, and settings live in `ui`; index types and tag sources
  come from `core` (e.g. future MusicBrainz lookups).
- The subprocess contract (args, stdout JSON schema, stderr log schema,
  exit codes) is specified in `CLI-CONTRACT.md` and is the only
  coupling between the CLI binary and the outside world.

## Open questions (decide before implementation)

- [x] Store dir name + index filename → `.files/` + `.index.json`.
- [x] Store names → `.files/<original-name>`, `_N` stem suffix on
      collision, hash-dupes purged at index time and at ingest.
- [x] Ingest (move into `.files/`) happens at `view`, not index time;
      `init` only creates `.index.json`; `update` only scans in place.
- [x] `update` is trusted by default (new files only); `--verify`
      additionally re-hashes every indexed path.
- [x] Nested `.index.json` files are verified and adopted with full
      metadata; failures keep the nested index and shield its files.
- [x] View semantics → ordered multi-key views; a record missing a
      level gets `Unsorted/` at that level, a record missing every
      level lands in one flat `Unsorted/`; cartesian product for
      multi-values.
- [x] Destructive safeguards → never interactive (no prompts, no stdin);
      `view` aborts on untracked files unless `--force` (destroys them);
      `--dry-run` everywhere mutating.
- [x] Tag charset → validated at entry (no `<>:"/\|?*,`, controls,
      trailing dots/spaces, reserved names); no view-time sanitization
      except case-fold `__2` suffix.
- [x] Link strategy → symlink, then hard link; failures are logged
      warnings (`linkErrors`), never copies — storey moves and deletes,
      never copies.
- [x] `rm`/`mv` derive link paths from tags (same mapping as `view`);
      links removed first, store file second.
- [x] Index writes → temp file + fsync + rename; `.index.lock`
      serializes writers across the whole read-modify-write span
      (readers never lock). Moved content repoints instead of
      purging (rename detection); stranded `.files/` content is
      reconciled by hash before any missing/prune judgment.
- [x] Crash safety → the index is saved right after file moves
      (links rebuild from it); SIGINT/SIGTERM saves first (exit
      130); `view` refuses filesystems with no link support after
      a pre-move probe.
- [x] Rescan problems → report-only, exit 1 when problems exist.
- [x] Hashing → worker pool, NumCPU-1 (one core left for UI/OS); progress as JSONL events
      on stderr for GUI consumption, streamed live (never batched at exit).
      Unreadable files are report-only problems, never fatal.
- [x] Scan hygiene → the walk skips OS junk (`.DS_Store`,
      `Thumbs.db`, `desktop.ini`) and partial downloads (`.part`,
      `.crdownload`) alongside `.files/`, `.index.json`, and
      symlinks.
- [x] Filenames → kept as-is, never normalized; storey only moves objects
      and manages links. New command: `storey mv` renames object + links.
