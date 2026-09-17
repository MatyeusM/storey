#!/usr/bin/env sh
# Print the most recent ## [x.y.z] section body of a Keep a Changelog
# file. A leading ## [Unreleased] is not a release and is skipped.
# Fails when no version section exists, so the release job never
# ships empty notes.
# Usage: sh scripts/release-notes.sh [changelog]
set -eu
file="${1:-CHANGELOG.md}"
body="$(awk '
  /^## \[/ {
    if (tolower($2) == "[unreleased]") next
    if (found) exit
    found = 1
    next
  }
  found { print }
' "$file")"
if [ -z "$body" ]; then
  echo "release-notes: no ## [x.y.z] section in $file" >&2
  exit 1
fi
printf '%s\n' "$body"
