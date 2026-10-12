# storey-gui

Rust + cxx-qt (Qt6) graphical interface. Thin client over the `storey`
CLI binary — all CAS/index/view logic stays in `apps/cli`; the GUI
shells out to it through `apps/shared` (see `docs/CLI-CONTRACT.md`).

Win11-style explorer: an icon-only top bar (init outside libraries,
rescan inside; up; `/Name` + Files|Folders toggle + view-tag
breadcrumbs in libraries, plain cwd elsewhere; search with an
untagged-only toggle; open-in-explorer), a quick-access side pane
(XDG places), a large-tile file grid (folders and files share one
grid; the files view hides folders), and a status footer under the
grid. Four modals: init confirm (Confirm preselected), plugin health
+ git-url install (with a trust confirm before code is fetched), the
view-tag picker (only addable categories), and the per-file tag
editor (always set mode, values prefilled from present tags) with
plugin suggestions.

## Layout

- `src/store.rs` — `StoreyLibrary` QObject via the cxx-qt bridge:
  explorer state as `QString`/`bool`/`i32`/`f64` properties (lists
  travel as JSON for `JSON.parse` in QML) and one `#[qinvokable]`
  per user action. Bodies live in the modules below so the bridge
  file stays small.
- `src/nav.rs` — configure, refresh, up/open/enter, `init`/`update`
  on a worker thread, open-in-explorer, status clearing (plus
  unconditional dismiss for the decaying hint). Entering a
  directory inside a library is blocked: the status bar reports
  `storey error: Use file explorer instead`, decaying after 10s.
- `src/state.rs` — synchronous re-sync (places, `status`,
  entries, records, counts), notices/errors in `status_text`
  only, progress ticks, init/update completion (problem exits
  still show their report summary).
- `src/library.rs` — shallow filter (names, plus tag content in a
  repo), untagged-only toggle (forces the file view; leaving for
  folders restores the lens), file-view switch, view-tag add/remove
  via `view --tags=…`, record/file selection for the tag editor.
- `src/tagging.rs` — tag set, plugin suggestions + apply-all;
  categories re-sync after writes so the folder-view [+] appears.
- `src/plugins.rs` — reload rows from `ui.toml`, enable toggles,
  git-url install (shallow clone into the config plugins dir).
- `src/support.rs` — Qt-free JSON builders, places, human sizes,
  plugin rows. Unit-tested (`cargo test`); the `.files/` store and
  all dotfiles are hidden from every listing.
- `qml/Main.qml` — explorer shell wiring the components below.
- `qml/Components/` — `TopBar`, `SidePane` (quick access),
  `FileGrid` (large tiles), `StatusBar` (state + counts, transient
  hint decay), `ConfirmDialog` (Confirm preselected),
  `PluginsDialog`, `TagEditorDialog`, `ViewTagDialog`.

## Theme (`qml/Theme/`, primitives in `qml/Ui/`)

Win11-like light tokens: `AppTheme` (background, surface, border,
text, muted, accent, danger), `Base` (rem unit), `Spacing`,
`Typography`, and `Icons` (hardcoded Nerd Font codepoints) scales,
plus the `Ui` primitives `Icon`, `IconButton`, and
`PrimaryButton` (subtle hover fills, rounded corners, pointer
cursors). `Main.qml` only assembles `qml/Components/` views, never
`Ui` directly. Singletons (`pragma Singleton` +
`QmlFile::singleton(true)` in `build.rs`); fonts ship as module
resources (`qml/Theme/fonts/`). `AppTheme` (not `Color`: that name
belongs to QtQuick since Qt 6) owns the defaults.

`Main.qml` applies `ui.toml` overrides at startup through the
`StoreyLibrary` (`Base`/`AppTheme`/`Typography` `.apply(library)`
read the explicit, valid `[gui]` keys from one JSON object):

```toml
[gui]
rem_px = 18
sans_family = "Inter"
```

Invalid values (bad hex, non-positive `rem_px`) keep the default —
see `storey-shared::theme`. Note: `QtObject` has no default
property, so singletons needing child objects (font loaders) must
use an `Item` root.

## QML gotchas (learned the hard way)

- `build.rs` must `println!("cargo::rerun-if-changed=...")` for every
  QML source: cxx-qt does not track them, so without it the binary
  silently keeps stale UI.
- Never put `implicitWidth`/`implicitHeight` on direct `Dialog`
  children — the scene fails to load with no QML warnings. Give each
  dialog an explicit `width` and a `contentItem` layout instead.
- Offscreen smoke test (hyprland gives 0x0 windows, so run headless;
  exit 124 = event loop running, exit 1 + "failed to load" = broken):
  `QT_QPA_PLATFORM=offscreen timeout 10 ./target/debug/storey-gui <dir>`.

## System dependency: Qt6 (not via mise)

`mise.toml` provides Go + Rust. Qt6 comes from the OS (needed at
build time by `cxx-qt-build`, which finds it via `qmake`):

```sh
# Arch
sudo pacman -S qt6-base qt6-declarative
# Ubuntu/Debian
sudo apt install qt6-base-dev qt6-declarative-dev
# macOS
brew install qt
# Windows: install Qt 6 via the online installer or `winget install Qt.Qt.6`,
# then ensure qmake is on PATH.
```

## Build / run

```sh
mise run build:gui          # debug build -> apps/gui/target/debug/storey-gui
mise run build:gui-release  # release build
mise run dev:gui            # fresh CLI pinned via --storey-bin
mise run test:gui           # cargo test
mise run lint:gui           # fmt check + clippy
```

`storey-gui [PATH] [--storey-bin BIN]`: binary resolution is flag,
then `ui.toml`, then a sibling `storey`, then PATH.
Version: `Cargo.toml` `version` must match repo-root `VERSION`.
