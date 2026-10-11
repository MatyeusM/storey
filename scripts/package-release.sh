#!/usr/bin/env sh
# Assemble per-OS release zips from downloaded CI artifacts, with
# canonical binary names so each platform is one obvious download:
# storey-linux.zip, storey-windows.zip, storey-macos.zip.
# Usage: sh scripts/package-release.sh <artifacts-dir> <out-dir>
set -eu
in="$1"
out="$2"
mkdir -p "$out"
out_abs="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

# linux (amd64 throughout)
mkdir -p "$stage/linux"
cp "$in/cli-linux-amd64/dist/storey-"*/linux-amd64/storey "$stage/linux/storey"
cp "$in/tui-ubuntu-latest/storey-tui" "$stage/linux/storey-tui"
cp "$in/gui-ubuntu-latest/storey-gui" "$stage/linux/storey-gui"
(cd "$stage/linux" && zip -q "$out_abs/storey-linux.zip" storey storey-tui storey-gui)

# windows (amd64 throughout)
mkdir -p "$stage/windows"
cp "$in/cli-windows-amd64/dist/storey-"*/windows-amd64/storey.exe "$stage/windows/storey.exe"
cp "$in/tui-windows-latest/storey-tui.exe" "$stage/windows/storey-tui.exe"
cp "$in/gui-windows-latest/storey-gui.exe" "$stage/windows/storey-gui.exe"
(cd "$stage/windows" && zip -q "$out_abs/storey-windows.zip" storey.exe storey-tui.exe storey-gui.exe)

# macos (TUI/GUI are arm64 runner builds; the CLI stays amd64)
mkdir -p "$stage/macos"
cp "$in/cli-darwin-amd64/dist/storey-"*/darwin-amd64/storey "$stage/macos/storey"
cp "$in/tui-macos-latest/storey-tui" "$stage/macos/storey-tui"
cp "$in/gui-macos-latest/storey-gui" "$stage/macos/storey-gui"
(cd "$stage/macos" && zip -q "$out_abs/storey-macos.zip" storey storey-tui storey-gui)

ls "$out"
