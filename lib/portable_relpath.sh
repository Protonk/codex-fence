#!/usr/bin/env bash

# Helper utilities that probes can source to access portable path helpers
# without duplicating interpreter detection logic. Keep these helpers pure (no
# global state or side effects) so probes remain single-purpose.

portable_relpath() { # compute a relative path without assuming a specific interpreter
  local target="$1" # path we want relative form of
  local base="$2"   # directory to make the path relative to
  if command -v python3 >/dev/null 2>&1; then # prefer python3 when available
    python3 - "$target" "$base" <<'PY' # run inline script to emit the relpath
import os  # provide os.path.relpath
import sys  # fetch positional arguments passed from the shell
print(os.path.relpath(sys.argv[1], sys.argv[2]))  # write the computed relative path
PY
    return # we already produced a result, no need to try other interpreters
  fi
  if command -v python >/dev/null 2>&1; then # fall back to python2 if needed
    python - "$target" "$base" <<'PY' # same logic for older Python installs
import os  # provide os.path.relpath
import sys  # fetch positional arguments passed from the shell
print(os.path.relpath(sys.argv[1], sys.argv[2]))  # write the computed relative path
PY
    return # stop after successfully printing the relative path
  fi
  if command -v perl >/dev/null 2>&1; then # Perl provides another portable option
    perl -MFile::Spec -e 'print File::Spec->abs2rel($ARGV[0], $ARGV[1])' "$target" "$base" # rely on File::Spec
    return # exit after Perl handles the conversion
  fi
  printf '%s' "${target}" # best-effort fallback: return the original target as-is
}
