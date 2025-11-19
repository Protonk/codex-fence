#!/usr/bin/env bash

# Helper utilities that probes can source to access portable path helpers
# without duplicating interpreter detection logic. Keep these helpers pure (no
# global state or side effects) so probes remain single-purpose.

portable_relpath() {
  local target="$1"
  local base="$2"
  if command -v python3 >/dev/null 2>&1; then
    python3 - "$target" "$base" <<'PY'
import os
import sys
print(os.path.relpath(sys.argv[1], sys.argv[2]))
PY
    return
  fi
  if command -v python >/dev/null 2>&1; then
    python - "$target" "$base" <<'PY'
import os
import sys
print(os.path.relpath(sys.argv[1], sys.argv[2]))
PY
    return
  fi
  if command -v perl >/dev/null 2>&1; then
    perl -MFile::Spec -e 'print File::Spec->abs2rel($ARGV[0], $ARGV[1])' "$target" "$base"
    return
  fi
  printf '%s' "${target}"
}
