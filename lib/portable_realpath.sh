#!/usr/bin/env bash

# Helper utilities that probes can source to access portable path helpers
# without duplicating interpreter detection logic. Keep these helpers pure (no
# global state or side effects) so probes remain single-purpose.

portable_realpath() {
  local target="$1"
  if command -v python3 >/dev/null 2>&1; then
    python3 - "$target" <<'PY'
import os
import sys
path = sys.argv[1]
try:
    print(os.path.realpath(path))
except OSError:
    print("")
PY
    return
  fi
  if command -v python >/dev/null 2>&1; then
    python - "$target" <<'PY'
import os
import sys
path = sys.argv[1]
try:
    print(os.path.realpath(path))
except OSError:
    print("")
PY
    return
  fi
  if command -v perl >/dev/null 2>&1; then
    perl -MCwd=abs_path -e 'my $p = shift; my $rp = eval { abs_path($p) }; print defined($rp) ? $rp : ""' "$target"
    return
  fi
  printf ''
}
