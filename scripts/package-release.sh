#!/usr/bin/env sh
# Assemble per-app, per-OS release archives from downloaded CI artifacts:
#   storey-<ver>-<os>-<arch>.tar.gz   (linux, macos)
#   storey-gui-<ver>-<os>-<arch>.tar.gz
#   storey-<ver>-windows-<arch>.zip    (Explorer opens zips natively)
#   storey-gui-<ver>-windows-<arch>.zip (ships its Qt bundle)
# Binaries are located by artifact + filename (never by assumed
# nesting: upload-artifact flattens top-level directories).
# Usage: sh scripts/package-release.sh <artifacts-dir> <out-dir> [version]
set -eu
in="$1"
out="$2"
ver="${3:-$(cat VERSION)}"
mkdir -p "$out"
out_abs="$(cd "$out" && pwd)"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

# Runner arch per OS (single table: extend when adding arm runners).
arch_for() {
  case "$1" in
    linux | windows) printf 'x86_64' ;;
    macos) printf 'aarch64' ;;
    *) echo "package-release: unknown os $1" >&2; exit 1 ;;
  esac
}

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

# Pack one app for one OS: tarball (linux/macos) or zip (windows).
pack() {
  app="$1"  # storey | storey-gui
  os="$2"   # linux | windows | macos
  src="$3"  # staged dir holding the payload
  arch="$(arch_for "$os")"
  if [ "$os" = "windows" ]; then
    (cd "$src" && zip -q -r "$out_abs/$app-$ver-$os-$arch.zip" .)
  else
    (cd "$src" && tar -czf "$out_abs/$app-$ver-$os-$arch.tar.gz" .)
  fi
}

# linux (GUI needs system Qt6; see README install notes)
mkdir -p "$stage/linux-cli" "$stage/linux-gui"
stage_cp cli-ubuntu-latest storey "$stage/linux-cli/storey"
stage_cp gui-ubuntu-latest storey-gui "$stage/linux-gui/storey-gui"
pack storey linux "$stage/linux-cli"
pack storey-gui linux "$stage/linux-gui"

# windows (GUI ships its Qt bundle)
mkdir -p "$stage/windows-cli" "$stage/windows-gui"
stage_cp cli-windows-latest storey.exe "$stage/windows-cli/storey.exe"
cp -r "$in/gui-windows-latest/." "$stage/windows-gui/"
pack storey windows "$stage/windows-cli"
pack storey-gui windows "$stage/windows-gui"

# macos (native arm64 throughout)
mkdir -p "$stage/macos-cli" "$stage/macos-gui"
stage_cp cli-macos-latest storey "$stage/macos-cli/storey"
stage_cp gui-macos-latest storey-gui "$stage/macos-gui/storey-gui"
pack storey macos "$stage/macos-cli"
pack storey-gui macos "$stage/macos-gui"

ls "$out"
