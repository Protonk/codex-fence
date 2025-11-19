#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Utility helpers sourced by every test suite. Centralized repo root detection
# and probe script parsing so far.
# 
# -----------------------------------------------------------------------------
set -euo pipefail

if [[ -z "${REPO_ROOT:-}" ]]; then
  # Resolve REPO_ROOT relative to this file so tests can be run from anywhere.
  REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." >/dev/null 2>&1 && pwd)
fi

extract_probe_var() {
  local file="$1"
  local var="$2"
  local line value trimmed first last value_length
  # Grab the first assignment so we mimic how probes declare constants.
  line=$(grep -E "^[[:space:]]*${var}=" "$file" | head -n1 || true)
  if [[ -z "${line}" ]]; then
    return 1
  fi
  # Strip inline comments + whitespace before removing wrapping quotes.
  value=${line#*=}
  value=${value%%#*}
  value=$(printf '%s' "${value}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  if [[ -n "${value}" ]]; then
    # Drop matching outer quotes without disturbing inner characters.
    first=${value:0:1}
    last=${value: -1}
    value_length=${#value}
    if [[ "${first}" == '"' && "${last}" == '"' && ${value_length} -ge 2 ]]; then
      value=${value:1:value_length-2}
    elif [[ "${first}" == "'" && "${last}" == "'" && ${value_length} -ge 2 ]]; then
      value=${value:1:value_length-2}
    fi
  fi
  printf '%s\n' "${value}"
}

resolve_probe_script_path() {
  local repo_root="$1"
  local identifier="$2"
  local attempts=() trimmed candidate
  if [[ -z "${identifier}" ]]; then
    return 1
  fi
  if [[ "${identifier}" == /* ]]; then
    # Absolute paths are trusted so tests can point directly at files.
    attempts+=("${identifier}")
  else
    trimmed=${identifier#./}
    # Search relative paths, auto-append .sh, then fall back to probes/.
    attempts+=("${repo_root}/${trimmed}")
    if [[ "${trimmed}" != *.sh ]]; then
      attempts+=("${repo_root}/${trimmed}.sh")
    fi
    attempts+=("${repo_root}/probes/${trimmed}")
    if [[ "${trimmed}" != *.sh ]]; then
      attempts+=("${repo_root}/probes/${trimmed}.sh")
    fi
  fi
  for candidate in "${attempts[@]}"; do
    # Only return files that actually live under probes/ to enforce the layout.
    if [[ -f "${candidate}" && "${candidate}" == "${repo_root}/probes/"* ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}
