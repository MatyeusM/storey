# AGENTS.md

## Workflow

- mise-first: `mise install` once, then `mise run build|test|lint` (per-app: `build:cli`, `test:shared`, `lint:gui`, ...). mise provides Rust (`latest`); **Qt6 (>= 6.6) is a system dep** for the GUI (`qt6-base` + `qt6-declarative`), found at build time via `qmake`.
- Task bodies must stay single-tool invocations with `run_windows` overrides — no implementation in `mise.toml`.
- Single version source: repo-root `VERSION`; keep the root
  `Cargo.toml` `[workspace.package]` version in sync (see `docs/RELEASING.md`).

## Layout

- `apps/cli` (Rust bin `storey`): thin CLI over `core` — arg parsing plus response printing only.
- `crates/ui` (Rust lib `storey-ui`): interface logic — Qt-free state and actions over `core` (settings, plugins, theming inputs). `apps/gui` is its thin Qt/QML shell.
- `crates/core` (Rust lib `core`): CLI logic — index, hashing, views, adoption (port of the Go CLI internals).
- `crates/meta` (Rust lib `storey-meta`): embedded file-metadata reads (audio, EXIF, PNG text) — local only, never network.
- `apps/gui` (Rust/cxx-qt 0.10): thin Qt/QML shell over `ui` (linked `core`, no subprocess). It **never touches the store or index directly** — all logic lives in crates.
- `.reference/` is a behavioral reference only — do not copy from it verbatim.
- Rust crates form one workspace (root `Cargo.toml`); single `Cargo.lock`
  at the root is committed. Member tasks still address crates directly:
  `cargo build --manifest-path apps/<crate>/Cargo.toml` from the repo root.
- Shared versions live in `[workspace.dependencies]`; members inherit
  (`dep.workspace = true`) plus the single `[workspace.package] version`
  (kept in sync with repo-root `VERSION`, see `docs/RELEASING.md`).
- The GUI build compiles Qt moc code (~1 min, only warnings expected); it needs Qt dev files + a C++ compiler, not just `cargo`.

## CLI contract (pinned in `docs/CLI-CONTRACT.md`)

- `init` creates **only** `.index.json`; `.files/` is created by `view`. `init` fails inside an existing library; `update` requires one.
- `update` is trusted by default (new files only); `--verify` additionally re-hashes everything. Problems are report-only, exit 1.
- Nested `.index.json` dirs are hash-verified and adopted with full metadata on `init`/`update`; failures keep the nested index and shield its files from fresh scans.
- Storey **never copies** file content (moves + deletes only); view links are symlink → hardlink, failures are logged warnings (`linkErrors`), never fatal.
- Index shape is `{"storey": {"schema": 1, ...}, ...}` — anything without the header is rejected. No legacy-format support.
- CLI is never interactive: no prompts, no stdin. Logs/progress go to **stderr** (human text, or JSONL under `--json`); stdout is reserved for the `--json` envelope.

## Size rules (all languages)

- Files ≤ 300 lines (via `scripts/check-limits.sh`), functions ≤ 50
  lines (via clippy `too-many-lines`, threshold in `clippy.toml`).
  Each `lint:*` task runs both for its own app.
  Keep new code split into small modules/packages.

## Rust rules

- `cargo fmt -- --check` and `cargo clippy -- -D warnings` must pass per crate (wired into `lint:*` tasks); run `mise run fmt` to fix.
- Focused test: `cargo test --manifest-path crates/ui/Cargo.toml <name>`.
