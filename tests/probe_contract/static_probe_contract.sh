#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Static probe contract test.
#
# This script enforces the structural rules every probe must follow without
# executing the probe. It performs:
#   - Syntax checking via `bash -n`.
#   - Contract validation (shebang, strict mode, probe_name/id, capability
#     metadata, emit-record flags, executable bit).
#
# When invoked with --probe <path|id> it checks a single script. Without
# arguments, it scans every probes/*.sh file. Fails on the first contract
# violation for each script and exits non-zero if any probe drifts.
# -----------------------------------------------------------------------------
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)

if [[ -f "${script_dir}/../library/utils.sh" ]]; then
  # shellcheck source=/dev/null
  source "${script_dir}/../library/utils.sh"
fi

if ! declare -F extract_probe_var >/dev/null 2>&1; then
  echo "static_probe_contract: missing tests/library/utils.sh helpers" >&2
  exit 1
fi

if [[ -z "${REPO_ROOT-}" ]]; then
  REPO_ROOT=$(cd "${script_dir}/../.." >/dev/null 2>&1 && pwd)
fi

portable_path_helper="${REPO_ROOT}/bin/portable-path"
if [[ ! -x "${portable_path_helper}" ]]; then
  echo "static_probe_contract: missing portable-path helper at ${portable_path_helper}" >&2
  exit 1
fi

portable_realpath() {
  "${portable_path_helper}" realpath "$1"
}

usage() {
  cat <<'USAGE' >&2
Usage: tests/probe_contract/static_probe_contract.sh [--probe <probe-id-or-path>]

Runs syntax + structural checks against one probe (with --probe) or against
every probes/*.sh script when no arguments are supplied.
USAGE
}

target_probe=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --probe)
      if [[ $# -lt 2 ]]; then
        usage
        exit 1
      fi
      if [[ -n "${target_probe}" ]]; then
        echo "static_probe_contract: only one --probe value is supported" >&2
        exit 1
      fi
      target_probe="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "static_probe_contract: unknown argument '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

resolve_probe() {
  local identifier="$1"
  local resolved=""
  if declare -F resolve_probe_script_path >/dev/null 2>&1; then
    resolved=$(resolve_probe_script_path "${REPO_ROOT}" "${identifier}" || true)
  else
    if [[ -f "${identifier}" ]]; then
      resolved=$(portable_realpath "${identifier}")
    elif [[ -f "${REPO_ROOT}/probes/${identifier}" ]]; then
      resolved=$(portable_realpath "${REPO_ROOT}/probes/${identifier}")
    elif [[ -f "${REPO_ROOT}/probes/${identifier}.sh" ]]; then
      resolved=$(portable_realpath "${REPO_ROOT}/probes/${identifier}.sh")
    fi
  fi
  printf '%s' "${resolved}"
}

bash_bin=""
if command -v bash >/dev/null 2>&1; then
  bash_bin="$(command -v bash)"
fi
if [[ -z "${bash_bin}" ]]; then
  echo "static_probe_contract: unable to locate bash interpreter for syntax checks" >&2
  exit 1
fi

collect_probes() {
  if [[ -n "${target_probe}" ]]; then
    local resolved
    resolved=$(resolve_probe "${target_probe}")
    if [[ -z "${resolved}" || ! -f "${resolved}" ]]; then
      echo "static_probe_contract: unable to resolve probe '${target_probe}'" >&2
      exit 1
    fi
    if [[ "${resolved}" != "${REPO_ROOT}/probes/"* ]]; then
      echo "static_probe_contract: '${resolved}' is outside probes/" >&2
      exit 1
    fi
    printf '%s\n' "${resolved}"
    return 0
  fi

  if [[ ! -d "${REPO_ROOT}/probes" ]]; then
    echo "static_probe_contract: probes/ directory not found" >&2
    exit 1
  fi

  find "${REPO_ROOT}/probes" -type f -name '*.sh' -print | LC_ALL=C sort | while read -r script; do
    portable_realpath "${script}"
  done
}

have_flag() {
  local script="$1"
  local flag="$2"
  if ! grep -q -- "${flag}" "${script}"; then
    return 1
  fi
  return 0
}

check_probe() {
  local probe_script="$1"
  local rel_path=${probe_script#"${REPO_ROOT}/"}
  local probe_file
  probe_file=$(basename "${probe_script}")
  local probe_id=${probe_file%.sh}
  local errors=()

  if [[ ! -x "${probe_script}" ]]; then
    errors+=("not executable (chmod +x)")
  fi

  local first_line
  first_line=$(head -n 1 "${probe_script}" || true)
  if [[ "${first_line}" != '#!/usr/bin/env bash' ]]; then
    errors+=("missing '#!/usr/bin/env bash' shebang")
  fi

  if ! grep -Eq '^[[:space:]]*set -euo pipefail' "${probe_script}"; then
    errors+=("missing 'set -euo pipefail'")
  fi

  local syntax_error
  if ! syntax_error=$("${bash_bin}" -n "${probe_script}" 2>&1); then
    syntax_error=${syntax_error:-"bash -n failed"}
    errors+=("syntax error: ${syntax_error}")
  fi

  local probe_name
  probe_name=$(extract_probe_var "${probe_script}" "probe_name" 2>/dev/null || true)
  if [[ -z "${probe_name}" ]]; then
    errors+=("missing probe_name assignment")
  elif [[ "${probe_name}" != "${probe_id}" ]]; then
    errors+=("probe_name '${probe_name}' does not match filename '${probe_id}'")
  fi

  local primary_capability
  primary_capability=$(extract_probe_var "${probe_script}" "primary_capability_id" 2>/dev/null || true)
  if [[ -z "${primary_capability}" ]]; then
    errors+=("missing primary_capability_id assignment")
  fi

  if ! grep -q 'emit_record_bin' "${probe_script}"; then
    errors+=("missing emit_record_bin helper")
  fi

  local required_flags=(
    '--run-mode'
    '--probe-name'
    '--probe-version'
    '--primary-capability-id'
    '--command'
    '--category'
    '--verb'
    '--target'
    '--status'
    '--payload-file'
    '--operation-args'
  )
  local flag
  for flag in "${required_flags[@]}"; do
    if ! have_flag "${probe_script}" "${flag}"; then
      errors+=("missing ${flag} in emit-record call")
    fi
  done

  if [[ ${#errors[@]} -eq 0 ]]; then
    echo "static_probe_contract: [PASS] ${rel_path}"
    return 0
  fi

  echo "static_probe_contract: [FAIL] ${rel_path}" >&2
  local err
  for err in "${errors[@]}"; do
    echo "  - ${err}" >&2
  done
  return 1
}

probe_list=()
while IFS= read -r script; do
  probe_list+=("${script}")
done < <(collect_probes)

if [[ ${#probe_list[@]} -eq 0 ]]; then
  echo "static_probe_contract: no probes found" >&2
  exit 1
fi

failures=0
for script in "${probe_list[@]}"; do
  if ! check_probe "${script}"; then
    failures=$((failures + 1))
  fi
done

if [[ ${failures} -gt 0 ]]; then
  echo "static_probe_contract: ${failures} probe(s) failed" >&2
  exit 1
fi

if [[ -n "${target_probe}" ]]; then
  echo "static_probe_contract: verified ${target_probe}" >&2
else
  echo "static_probe_contract: verified ${#probe_list[@]} probe(s)" >&2
fi

exit 0
