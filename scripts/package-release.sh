#!/usr/bin/env sh
# Assemble per-OS release zips from downloaded CI artifacts, with
# canonical binary names so each platform is one obvious download:
# storey-linux.zip, storey-windows.zip, storey-macos.zip.
# Binaries are located by artifact + filename (never by assumed
# nesting: upload-artifact flattens top-level directories).
# Usage: sh scripts/package-release.sh <artifacts-dir> <out-dir>
set -eu
in="$1"
out="$2"
mkdir -p "$out"
out_abs="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

# Locate exactly one file by artifact dir + basename; fail loudly
# with the visible tree instead of a bare cp error.
locate() {
  result="$(find "$in/$1" -name "$2" -type f | sort | head -n 1)"
  if [ -z "$result" ]; then
    echo "package-release: missing $2 under $in/$1; tree:" >&2
    find "$in/$1" | head -20 >&2
    exit 1
  fi
  printf '%s' "$result"
}

# Stage one binary: locate it first so a miss fails here with the
# visible tree, instead of a bare cp error downstream.
stage_cp() {
  src="$(locate "$1" "$2")" || exit 1
  cp "$src" "$3"
}

# linux (amd64 throughout)
mkdir -p "$stage/linux"
stage_cp cli-linux-amd64 storey "$stage/linux/storey"
stage_cp tui-ubuntu-latest storey-tui "$stage/linux/storey-tui"
stage_cp gui-ubuntu-latest storey-gui "$stage/linux/storey-gui"
(cd "$stage/linux" && zip -q "$out_abs/storey-linux.zip" storey storey-tui storey-gui)

# windows (amd64 throughout)
mkdir -p "$stage/windows"
stage_cp cli-windows-amd64 storey.exe "$stage/windows/storey.exe"
stage_cp tui-windows-latest storey-tui.exe "$stage/windows/storey-tui.exe"
stage_cp gui-windows-latest storey-gui.exe "$stage/windows/storey-gui.exe"
(cd "$stage/windows" && zip -q "$out_abs/storey-windows.zip" storey.exe storey-tui.exe storey-gui.exe)

# macos (TUI/GUI are arm64 runner builds; the CLI stays amd64)
mkdir -p "$stage/macos"
stage_cp cli-darwin-amd64 storey "$stage/macos/storey"
stage_cp tui-macos-latest storey-tui "$stage/macos/storey-tui"
stage_cp gui-macos-latest storey-gui "$stage/macos/storey-gui"
(cd "$stage/macos" && zip -q "$out_abs/storey-macos.zip" storey storey-tui storey-gui)

ls "$out"
