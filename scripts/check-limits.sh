#!/usr/bin/env sh
# Enforce size limits: files <= 300 lines, functions <= 50 lines.
# Applies to Go (apps/cli) and Rust (apps/tui, apps/shared, apps/gui).
# Takes directories to scan; defaults to all four apps.
# Function length uses a brace-count heuristic, so unbalanced braces in
# string literals or comments can fool it — keep code clean instead of
# working around the checker.
set -eu
max_file=300
max_func=50
dirs="${*:-apps/cli apps/tui apps/shared apps/gui}"
bad=0
files=$(find $dirs \( -name '*.go' -o -name '*.rs' \) -not -path '*/bin/*' -not -path '*/target/*' | sort)
for f in $files; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$max_file" ]; then
    echo "too long ($n > $max_file): $f"
    bad=1
  fi
  out=$(awk -v max="$max_func" '
    /^func / {
      if (infunc) { report() }
      name=$0; start=NR; depth=0; infunc=1
    }
    /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]/ {
      if (infunc) { report() }
      name=$0; start=NR; depth=0; infunc=1
    }
    infunc {
      line=$0
      depth += gsub(/{/, "{", line) - gsub(/}/, "}", line)
      if (NR > start && depth <= 0) { report() }
    }
    function report() {
      len=NR-start+1
      if (len > max) { printf "%s:%d: %s (%d lines)\n", FILENAME, start, name, len }
      infunc=0
    }
    END { if (infunc) { report() } }
  ' "$f")
  if [ -n "$out" ]; then
    echo "$out"
    bad=1
  fi
done
if [ "$bad" -eq 0 ]; then
  echo "size limits ok"
else
  exit 1
fi
