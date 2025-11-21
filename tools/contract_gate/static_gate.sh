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
gate_name="contract_static_gate"

pathtools_path="${script_dir}/../../tools/pathtools.sh"
if [[ -f "${pathtools_path}" ]]; then
  # shellcheck source=/dev/null
  source "${pathtools_path}"
else
  echo "${gate_name}: missing tools/pathtools.sh helper" >&2
  exit 1
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

if ! declare -F resolve_probe_script_path >/dev/null 2>&1; then
  echo "${gate_name}: missing resolve_probe_script_path helper" >&2
  exit 1
fi

if [[ -z "${REPO_ROOT-}" ]]; then
  REPO_ROOT=$(cd "${script_dir}/../.." >/dev/null 2>&1 && pwd)
fi

if ! declare -F portable_realpath >/dev/null 2>&1; then
  echo "${gate_name}: missing portable_realpath helper" >&2
  exit 1
fi
usage() {
  cat <<'USAGE' >&2
Usage: tools/contract_gate/static_gate.sh [--probe <probe-id-or-path>]

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
        echo "${gate_name}: only one --probe value is supported" >&2
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
      echo "${gate_name}: unknown argument '$1'" >&2
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
  echo "${gate_name}: unable to locate bash interpreter for syntax checks" >&2
  exit 1
fi

collect_probes() {
  if [[ -n "${target_probe}" ]]; then
    local resolved
    resolved=$(resolve_probe "${target_probe}")
    if [[ -z "${resolved}" || ! -f "${resolved}" ]]; then
      echo "${gate_name}: unable to resolve probe '${target_probe}'" >&2
      exit 1
    fi
    if [[ "${resolved}" != "${REPO_ROOT}/probes/"* ]]; then
      echo "${gate_name}: '${resolved}' is outside probes/" >&2
      exit 1
    fi
    printf '%s\n' "${resolved}"
    return 0
  fi

  if [[ ! -d "${REPO_ROOT}/probes" ]]; then
    echo "${gate_name}: probes/ directory not found" >&2
    exit 1
  fi

  find "${REPO_ROOT}/probes" -type f -name '*.sh' -print | LC_ALL=C sort | while read -r script; do
    portable_realpath "${script}"
  done
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

  if [[ ${#errors[@]} -eq 0 ]]; then
    echo "${gate_name}: [PASS] ${rel_path}"
    return 0
  fi

  echo "${gate_name}: [FAIL] ${rel_path}" >&2
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
  echo "${gate_name}: no probes found" >&2
  exit 1
fi

failures=0
for script in "${probe_list[@]}"; do
  if ! check_probe "${script}"; then
    failures=$((failures + 1))
  fi
done

if [[ ${failures} -gt 0 ]]; then
  echo "${gate_name}: ${failures} probe(s) failed" >&2
  exit 1
fi

if [[ -n "${target_probe}" ]]; then
  echo "${gate_name}: verified ${target_probe}" >&2
else
  echo "${gate_name}: verified ${#probe_list[@]} probe(s)" >&2
fi

exit 0
