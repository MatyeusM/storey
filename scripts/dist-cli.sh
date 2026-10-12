#!/usr/bin/env sh
# Cross-compile the storey CLI for the release matrix into dist/.
# amd64 only for now (arm64 re-adds cleanly by extending the loop).
# Usage: sh scripts/dist-cli.sh [version]
set -eu
ver="${1:-$(cat VERSION)}"
out="dist/storey-${ver}"
mkdir -p dist
for os in linux windows darwin; do
  bin="storey"
  [ "$os" = "windows" ] && bin="storey.exe"
  dest="$out/$os-amd64/$bin"
  mkdir -p "$(dirname "$dest")"
  echo "building $os/amd64 -> $dest"
  GOOS="$os" GOARCH=amd64 go -C cli_go build \
    -ldflags "-X github.com/matyeusm/storey/internal/version.Version=$ver" \
    -o "../../$dest" ./cmd/storey
done
echo "artifacts under $out"
