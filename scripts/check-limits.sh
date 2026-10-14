#!/usr/bin/env sh
# Enforce file size limit: files <= 300 lines.
# Function length is clippy's job (`too-many-lines-threshold = 50`
# in clippy.toml, denied in the lint:* tasks). Applies to Rust
# (crates/*, apps/*). Takes directories to scan.
set -eu
max_file=300
dirs="${*:-crates apps/gui}"
bad=0
files=$(find $dirs -name '*.rs' -not -path '*/bin/*' -not -path '*/target/*' | sort)
for f in $files; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$max_file" ]; then
    echo "too long ($n > $max_file): $f"
    bad=1
  fi
done
if [ "$bad" -eq 0 ]; then
  echo "size limits ok"
else
  exit 1
fi
