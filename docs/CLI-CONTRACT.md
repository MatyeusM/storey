# CLI contract (v0.2 draft — decided, implemented)

`storey` owns all CAS/index/view logic. External tools drive it as
a subprocess using `--json` output (the GUI links the same logic
in-process instead). This file pins the command surface.

## Library layout (locked)

- `.files/` — content store, created by `view` (not by `init`):
  `<original-name>` flat; a name taken by different content gets a `_N`
  stem suffix (`movie_1.mkv`); files whose hash is already stored are
  duplicates and are purged — unless the indexed file is gone from
  disk, which means the content moved: the record is repointed
  (tags kept) instead, so renames are detected, never deleted.
- `.index.json` — tags, hashes, and view state. Schema v1, namespaced
  under a `storey` header object so foreign `.index.json` files are
  never mistaken for ours:
  `{"storey": {"schema": 1, "algorithm": "sha256"}, "view": ..., "files": [...]}`.
- `.index.lock` — write lock held across each mutating command's full
  read-modify-write span, so concurrent writers fail loudly instead of
  losing each other's records. Readers never lock. A leftover lock
  (killed process) needs a manual remove, which the error names.
  Anything without the header is rejected, never silently misread.
- Everything else in the library root is a **view** (regenerable).
- **Nested libraries** — a subdirectory holding its own `.index.json`
  is a nested library. `init` and `update` fold it into the parent:
  every record is hash-verified, then adopted with all metadata intact
  (in-place files stay put with rebased paths; stored objects move
  into the parent `.files/`). The nested `.index.json` is deleted only
  when all of its records adopt cleanly. Failed nested libraries keep
  their index and their files are shielded from fresh scans, so the
  user can repair and re-run instead of losing metadata.
- Storey never copies file content: it moves files, deletes duplicates
  and view artifacts, and links views. It never duplicates bytes.

## Invocation

- Library root = nearest ancestor of cwd (including cwd) containing
  `.index.json`. All commands except `init` require one; otherwise exit 2.
- Filenames are kept as-is: storey never rewrites content or normalizes
  names — it only moves objects and creates/removes links.
- `<target>` (for `tag`, `rm`, `mv`) = either a path inside the library (view
  link or store object — symlinks are resolved to the store object; if the
  resolved content matches no indexed object, it is an error) or a full
  SHA-256 hex / unambiguous prefix. Ambiguous prefix → exit 1, candidates
  listed on stderr (as `data` under `--json`).
- Global flags: `--help`, `--version`, `--json`, `--quiet`/`-q`.
  `--json` and `--quiet` may appear before or after the command
  (`update --verify --json` works like `--json update --verify`).
- `--json`: stdout is one JSON object
  `{"schema": 1, "command": "<name>", "data": ...}`; logs/diagnostics go
  to stderr. Without `--json`, human-readable text.
- Logging: everything except the `--json` envelope goes to stderr.
  Human mode prints `storey: ...` lines (`warning:` prefix for warnings),
  including one progress line per hashed file. Under `--json`, stderr
  is JSONL for GUI consumption: `{"level":"info","msg":"..."}`,
  `{"level":"warn","msg":"..."}`, and hashing progress
  `{"level":"progress","op":"hash"|"verify","done":n,"total":m,"path":"..."}`.
  `--quiet` suppresses info and progress lines (warnings still print).
- Exit codes: `0` ok (including empty `list`), `2` usage / no library,
  `1` runtime failure, `130` interrupted (the index is saved first).
- Never interactive: storey never prompts and never reads stdin. Command
  it, it does — every command must
  be fully determined by its flags. `--dry-run` previews without mutating.

## Tag and category names

Tags are validated at `tag` time so they are always view-safe — there is
no view-time sanitization. Category names and values must: be non-empty
after trimming; contain no `<>:"/\|?*,` and no control characters; not end
with a dot or space; not be a reserved name (`CON`, `PRN`, `AUX`, `NUL`,
`COM1`–`COM9`, `LPT1`–`LPT9`, case-insensitive). Commas are banned because
they separate values. Violations are usage errors (exit 2).

## Commands

### `storey init [--json]`

Creates `.index.json` in cwd — and only that file. Then folds in nested
libraries (verify + adopt, see Library layout) and recursively scans
cwd. Files stay in place — nothing moves until `view`, and `.files/`
is not created yet. Regular files are hashed (SHA-256, with progress)
and recorded with empty tags; symlinks are skipped. A file whose hash
is already indexed is a duplicate and is deleted immediately — unless
the indexed file is gone from disk, which means the content moved:
the record is repointed (tags kept), never deleted. Errors
(exit 1) if `.index.json` exists in cwd or any ancestor ("already
inside a library"); nested `.index.json` files are adopted, not
rejected. Verification failures exit 1 with the problem list in the
output. Does not materialize a view. `--dry-run` previews intake
without adopting, writing, or deleting anything.

### `storey update [--verify] [--dry-run] [--json]`

Requires a library (an `.index.json` in cwd or an ancestor).
Folds in nested libraries first, then scans for new files: unindexed
files stay in place and are hashed (with progress) and recorded; a
file whose hash is already indexed is a duplicate and is deleted
immediately — unless the indexed file is gone from disk, which means
the content moved: the record is repointed (tags kept), never deleted. Default is **trusted**: indexed files are not
re-hashed. With `--verify`, every indexed path is additionally
re-hashed and mismatches/absences are reported — except missing files,
which are pruned from the index at the end of the run instead (the
user may have renamed or deleted them; the entry is dropped, listed
under `removed`, and stale view links heal on the next `view`).
Before re-hashing, stranded `.files/` content is reconciled by hash
(same rule as `view` step 0), so interrupted runs never lose tags to
pruning.
Report-only otherwise: no repairs, no quarantining. If any problems
remain the command exits 1 with the problem list in the output.
`--dry-run` previews without adopting, writing, or deleting anything
(composes with `--verify`).

### `storey view --tags=<a,b,...> [--force] [--dry-run] [--json]`

`--tags` is required but may be empty: ordered comma-separated
categories, e.g. `--tags=date,actor` → `<date>/<actor>/movie.mkv`.
`--tags=` renders an empty directory — everything is ingested into
`.files/` and no links are built, recording an empty view.
The move into the store happens here, not at index time.

0. Reconcile — files stranded in `.files/` by an interrupted run are
   matched by hash against records whose location is gone and
   repointed (tags kept); unmatchable ones abort like untracked
   unless `--force` is given. Recovery precedes judgment.
1. Untracked check — regular files that are neither indexed nor system
   entries abort the command with their paths listed, unless `--force`
   is given (with `--force` they are destroyed; the warning names them).
2. Ingest — `.files/` is created, then each indexed file is moved into
   it under its display name; a file whose hash is already stored is a
   duplicate and is deleted instead. A name taken by different content
   gets a stem suffix (`movie.mkv` → `movie_1.mkv`, then `_2`, ...) in
   index order. The index is saved right after the moves, before
   anything below, so an abort still leaves a loadable index.
3. Deletes every remaining entry in the library root except `.files/`,
   `.index.json`, and `.index.lock` (symlinks unlinked, never
   followed).
4. Rebuilds nested folders, one link per object:
   - Missing value at a level → `Unsorted/` at that level, except a
     record missing every level lands in one flat `Unsorted/`.
   - Multiple values at a level → linked under each (cartesian product
     across levels).
   - Link strategy: symlink preferred, then hard link. Before moving
     anything, `view` probes the store for both and refuses with a
     plain message when the filesystem supports neither (exFAT/FAT32,
     Windows without developer mode, cross-volume stores). No copy
     fallback: when both fail the missing link is logged as a warning
     (stderr, `warn` JSONL under `--json`), recorded in `linkErrors`,
     and the view builds without it. Storey never copies content.
   - Names need no sanitization: tags are validated at entry (see Tag and
     category names). The only view-time fixup is a deterministic `__2`,
     `__3`, ... suffix when two values at the same level differ only by
     case (they collide on case-insensitive filesystems), recorded in the
     index so rebuilds are stable.
5. Records the view key order in `.index.json`. (Subsequent `view` runs
   skip step 2 for files already stored.)

`--dry-run` prints the planned tree/changes without touching disk
(machine-readable under `--json`).

### `storey status [--json]`

Read-only overview for humans and UIs: the library root, how many
files are indexed, how many are stored, how many wait on disk, and
the active view key order. No hashing, no mutation — safe to poll.
Outside a library it prints `fatal: not a storey repository` to
stderr and exits 2 (like git).

### `storey list [--untagged] [category=value ...] [--json]`

Lists indexed files. Default: all, as a human-readable table; `--json`
emits the record array.

- At most one `category=value` filter (split on the first `=`; quote
  values containing spaces/commas for the shell). More than one is a
  usage error for now — multi-filter AND/OR logic is deferred.
- `--untagged` = objects with no tags at all; cannot be combined with a
  `category=value` filter (exit 2).

### `storey tag <verb> ... [--json]`

Reads live under `get`, writes under `set`/`add`/`remove`.
`--category` and `--values` are the uniform operand names: `--values`
is comma-separated, stored values are always unique (duplicates
collapsed). Category and value strings must satisfy the tag
charset above; violations are usage errors (exit 2).

| Invocation | Effect |
| ---------- | ------ |
| `tag get` | list distinct categories, sorted |
| `tag get T` | tags map for one file |
| `tag get T --category C` | values for one category (`[]` when absent) |
| `tag set T --category C --values a,b` | set `C` to exactly `{a, b}` |
| `tag add T --category C --values a,b` | add `a`, `b` to `C`, keep existing |
| `tag remove T --category C --values a` | remove value `a` from `C` |
| `tag remove T --category C` | drop category `C` entirely |
| `tag remove T` | drop **all** tags on `T` |

`tag get` with `--values` is a usage error (exit 2); `tag set`/`add`
require both `--category` and `--values`; `tag remove` with `--values`
but no `--category` is a usage error (exit 2).

### `storey remove|rm <target> [--dry-run] [--json]`

Permanently deletes the object. Link paths are derived from the object's
tags (same mapping as `view`) and removed first; then the store file and
index record. `--dry-run` reports what would be deleted.

### `storey mv <target> <new-name> [--dry-run] [--json]`

Renames the object's display name: the store file is renamed, the index
record's name is updated, and all view links are re-created under the new
basename at the same tag-derived paths (old links removed first — same
mapping as `view`/`rm`). If the new name is taken by different content,
the stem-suffix rule applies (`_1`, ...). Before the first `view` (file
still in place) only the file and the index record are renamed.
`<new-name>` is a bare filename (no `/` or `\`, not `.`/`..`); otherwise
the tag-charset legality rules apply. Nothing is deleted (`mv` back
reverses it). `--dry-run` reports the planned renames.

### `storey restore [--force] [--dry-run] [--json]`

Undoes `view`: removes view artifacts and moves every stored object back
to its `originalRel`, recreating subdirectories. Records with no
`storedRel` are skipped (already home).

- Regular files that are neither tracked nor derived view artifacts abort
  the command with their paths listed, unless `--force` (destroys them).
- An existing file at a record's `originalRel` is a conflict: abort
  unless `--force` (overwrites it).
- Missing store objects abort before anything is mutated.
- Clears the recorded view key order.

## Record shape (v1, locked)

`{"id": "<sha256>", "displayName": "movie.mkv",
"originalRel": "films/movie.mkv", "storedRel": ".files/movie.mkv",
"size": n, "tags": {"actor": ["X"]}}` — `storedRel` is `""` until `view`
ingests the file; `tags` maps category → unique value array.

## JSON `data` sketches (v1)

- `init`: `{"added": [...], "purged": [...], "moved": [...],
  "adopted": [...], "problems": [...]}` (ids recorded / dupes deleted /
  moved content repointed / nested records adopted / failures).
- `update`: `{"added": [...], "purged": [...], "moved": [...],
  "adopted": [...], "removed": [...], "reverified": n, "problems": [...]}`
  (`reverified` is 0 without `--verify`; `removed` holds ids pruned
  as missing).
- `view`: `{"by": ["date","actor"], "moved": n, "purged": n,
  "untracked": [...], "orphans": [...], "links": n, "linkErrors": [...],
  "unsorted": n}`
  (+ `plan` link paths and `moves` src→dst map under `--dry-run`).
- `restore`: `{"restored": n, "skipped": n, "conflicts": [...],
  "problems": [...], "untracked": [...]}` (+ `moves` under `--dry-run`).
- `list`: array of records (see Record shape).
- `status`: `{"root": "/abs", "indexed": n, "stored": n,
  "untracked": n, "view": [...]}`.
- `tag`: `get` returns a category list, a tags map, or a value list
  by arity; `set`/`add`/`remove` return the updated record. `rm`: `{"removed": "<id>"}`.
  `mv`: `{"id": "...", "old": "a.mkv", "new": "b.mkv"}`.

Field names are locked at v1 (`schema: 1`); changing any `data`
shape bumps `schema`.
