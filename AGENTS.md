# AGENTS.md

## Workflow

- mise-first: `mise install` once, then `mise run build|test|lint` (per-app: `build:cli`, `test:shared`, `lint:gui`, ...). mise provides Go + Rust (`latest`); **Qt6 (>= 6.6) is a system dep** for the GUI (`qt6-base` + `qt6-declarative`), found at build time via `qmake`.
- Task bodies must stay single-tool invocations with `run_windows` overrides — no implementation in `mise.toml`.
- Single version source: repo-root `VERSION`; keep in sync with `cli_go/internal/version/version.go`, `crates/storey-shared/Cargo.toml`, `apps/gui/Cargo.toml` (see `docs/RELEASING.md`).

## Layout

- `cli_go` (Go reference, stdlib only): behavioral source of truth — CAS + index + views. Being ported to `crates/storey-core` + Rust `apps/cli`; delete once the user signs off on parity.
- `apps/cli` (reserved): future Rust CLI binary over `storey-core`.
- `crates/storey-shared` (Rust lib `storey-shared`): common GUI layer — CLI subprocess driver, log parsing, record model, tag sources.
- `crates/storey-core` (Rust lib `storey-core`): CLI logic — index, hashing, views, adoption (port of the Go CLI internals).
- `apps/gui` (Rust/cxx-qt 0.10): thin client. It shells out to `storey --json` and **never touches the store or index directly**.
- `.reference/` is a behavioral reference only — do not copy from it verbatim.
- Rust crates are standalone (no workspace); each `Cargo.lock` is committed. Always pass `--manifest-path apps/<crate>/Cargo.toml` from the repo root.
- The GUI build compiles Qt moc code (~1 min, only warnings expected); it needs Qt dev files + a C++ compiler, not just `cargo`.

## CLI contract (pinned in `docs/CLI-CONTRACT.md`)

- `init` creates **only** `.index.json`; `.files/` is created by `view`. `init` fails inside an existing library; `update` requires one.
- `update` is trusted by default (new files only); `--verify` additionally re-hashes everything. Problems are report-only, exit 1.
- Nested `.index.json` dirs are hash-verified and adopted with full metadata on `init`/`update`; failures keep the nested index and shield its files from fresh scans.
- Storey **never copies** file content (moves + deletes only); view links are symlink → hardlink, failures are logged warnings (`linkErrors`), never fatal.
- Index shape is `{"storey": {"schema": 1, ...}, ...}` — anything without the header is rejected. No legacy-format support.
- CLI is never interactive: no prompts, no stdin. Logs/progress go to **stderr** (human text, or JSONL under `--json`); stdout is reserved for the `--json` envelope.

## Size rules (all languages)

- Files ≤ 300 lines, functions ≤ 50 lines — enforced by `scripts/check-limits.sh`
  (Go + Rust; brace-count heuristic, so keep braces out of strings/comments).
  Each `lint:*` task runs it for its own app: `sh scripts/check-limits.sh apps/<app>`.
  Keep new code split into small modules/packages.

## Go rules

- Stdlib only (no third-party dependencies).
- Verify with `go -C cli_go vet ./... && go -C cli_go test ./...`; focused: `go -C cli_go test ./internal/index/ -run TestLoad -v`.
- Tests use `chdir(t, root)` + `mustRun(t, ...)` helpers (`cmd/storey/*_test.go`); use `testLogger()` (quiet) for direct `cmdX` calls so progress output doesn't pollute test logs.

## Rust rules

- `cargo fmt -- --check` and `cargo clippy -- -D warnings` must pass per crate (wired into `lint:*` tasks); run `mise run fmt` to fix.
- Focused test: `cargo test --manifest-path crates/storey-shared/Cargo.toml <name>`.
