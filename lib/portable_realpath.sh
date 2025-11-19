#!/usr/bin/env bash

# Helper utilities that probes can source to access portable path helpers
# without duplicating interpreter detection logic. Keep these helpers pure (no
# global state or side effects) so probes remain single-purpose.

portable_realpath() { # resolve the canonical absolute path for the provided target
  local target="$1" # path we want an absolute, resolved version of
  if command -v python3 >/dev/null 2>&1; then # prefer python3 for consistency
    python3 - "$target" <<'PY' # inline script computes a canonical path
import os  # give access to os.path.realpath
import sys  # read the argument passed from the shell
path = sys.argv[1]  # capture the target path
try:
    print(os.path.realpath(path))  # emit the fully resolved path
except OSError:
    print("")  # emit empty string if resolution failed
PY
    return # success, so no need to try other interpreters
  fi
  if command -v python >/dev/null 2>&1; then # fall back to whatever "python" points to
    python - "$target" <<'PY' # re-use the same inline helper
import os  # give access to os.path.realpath
import sys  # read the argument passed from the shell
path = sys.argv[1]  # capture the target path
try:
    print(os.path.realpath(path))  # emit the fully resolved path
except OSError:
    print("")  # emit empty string if resolution failed
PY
    return # stop after emitting the resolved path
  fi
  if command -v perl >/dev/null 2>&1; then # Perl fallback mirrors the Python logic
    perl -MCwd=abs_path -e 'my $p = shift; my $rp = eval { abs_path($p) }; print defined($rp) ? $rp : ""' "$target" # emit resolved path or empty string
    return # exit as soon as Perl finishes
  fi
  printf '' # best-effort fallback: emit empty string when no interpreter exists
}
