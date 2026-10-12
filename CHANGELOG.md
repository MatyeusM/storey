# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed

- TUI (`storey-tui`): the terminal client is gone. `apps/tui` and
  its `build/test/lint/dev:tui` tasks, CI job, and release
  artifacts are removed; the monorepo is CLI + GUI only.

### Fixed

- CI installs `rustfmt`/`clippy` into the active toolchain before
  linting, so fresh runners no longer fail `lint:*` on missing
  components.
- Releases ship per-OS zips (`storey-linux.zip`,
  `storey-windows.zip`, `storey-macos.zip`) instead of a flat,
  colliding file list; stray build files (`.d`) no longer upload.

## [0.1.0] - 2026-10-11

### Added

- CLI (`storey`): `init`, `update` (`--verify`, `--dry-run`), `view`
  (`--force`, `--dry-run`), `restore`, `rm`, `mv`, `tag`, `list`,
  `status` — all with `--json` envelopes and live stderr progress.
- Content tracking: hash-identified records, duplicate purging, and
  move/rename detection that repoints records (tags kept) instead of
  deleting.
- Crash safety: `.index.lock` writer serialization, index saved right
  after file moves, SIGINT/SIGTERM saves first (exit `130`), and
  stranded store content reconciled by hash before any prune.
- View robustness: pre-move filesystem link probe, OS junk and
  partial-download ignore list, one flat `Unsorted` for fully
  untagged files.
- TUI (ratatui): browse/files/detail/plugins views, background ops
  with progress, tag editing, plugin suggestions.
- GUI (Qt6): explorer shell with Files|Folders toggle, view-tag
  breadcrumbs, tag editor with prefill and suggestions, plugin
  install with trust confirm.
- Plugins: external suggester protocol (record JSON on stdin,
  suggestion array on stdout) via `ui.toml` plus `manifest.json`;
  documented in `docs/PLUGINS.md`.
- Release tooling: per-OS builds with checksums, patch notes taken
  from this file (`scripts/release-notes.sh`).

### Fixed

- `view`/`restore` JSON payloads emit `[]`, never `null`, for empty
  lists.
- `restore` collects every file in shared view directories (a
  `SkipDir`-on-file walk bug left siblings behind).
