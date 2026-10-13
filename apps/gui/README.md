# storey-gui

Rust + cxx-qt (Qt6) graphical interface. Thin Qt shell over the
`ui` crate, which holds all Qt-free state and actions over linked
`core` — no subprocess, no `storey` binary needed.

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
  per user action. Each body moves the inner `ui::State` out,
  calls one `ui` action, and commits the mirror back.
- `src/nav.rs`, `src/library.rs`, `src/tagging.rs`, `src/plugins.rs`
  — one thin `impl` block each delegating to `ui`; `init`/`update`
  run linked `core` flows on a worker thread with live progress.
- `src/support.rs` — QString read helpers only. All logic (JSON
  builders, filtering, locks, progress labels) lives in `ui`.
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
see `ui::theme`. Note: `QtObject` has no default
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

`mise.toml` provides Rust. Qt6 comes from the OS (needed at
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
mise run build:gui          # debug build -> target/debug/storey-gui
mise run build:gui-release  # release build
mise run dev:gui            # run from source
mise run test:gui           # cargo test
mise run lint:gui           # fmt check + clippy
```

`storey-gui [PATH]`: opens the directory (defaults to cwd).
Version: workspace `Cargo.toml` `version` must match repo-root `VERSION`.
