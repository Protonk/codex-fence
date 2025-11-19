#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${REPO_ROOT:-}" ]]; then
  REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." >/dev/null 2>&1 && pwd)
fi

extract_probe_var() {
  local file="$1"
  local var="$2"
  local line value trimmed first last value_length
  line=$(grep -E "^[[:space:]]*${var}=" "$file" | head -n1 || true)
  if [[ -z "${line}" ]]; then
    return 1
  fi
  value=${line#*=}
  value=${value%%#*}
  value=$(printf '%s' "${value}" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')
  if [[ -n "${value}" ]]; then
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
    attempts+=("${identifier}")
  else
    trimmed=${identifier#./}
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
    if [[ -f "${candidate}" && "${candidate}" == "${repo_root}/probes/"* ]]; then
      printf '%s\n' "${candidate}"
      return 0
    fi
  done
  return 1
}
