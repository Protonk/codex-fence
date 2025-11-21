#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="agent_approvals_mode_env"
primary_capability_id="cap_agent_approvals_mode"
tmp_stage_dir="${repo_root}/tmp/${probe_name}"
mkdir -p "${tmp_stage_dir}" 2>/dev/null || true

payload_file=""

cleanup() {
  if [[ -n "${payload_file}" ]]; then
    rm -f "${payload_file}" || true
  fi
}
trap cleanup EXIT

candidate_vars=(
  "FENCE_APPROVALS_MODE"
  "CODEX_APPROVALS_MODE"
  "CODEX_APPROVAL_MODE"
  "CODEX_APPROVALS"
  "CODEX_PERMISSIONS_MODE"
)

regex='^(FENCE_APPROVALS_MODE|CODEX_APPROVALS_MODE|CODEX_APPROVAL_MODE|CODEX_APPROVALS|CODEX_PERMISSIONS_MODE)='
printf -v command_executed "env | grep -m 1 -E %q" "${regex}"

candidate_vars_json=$(printf '%s\n' "${candidate_vars[@]}" | jq -R . | jq -s .)
operation_args=$(jq -n \
  --arg regex "${regex}" \
  --argjson candidate_vars "${candidate_vars_json}" \
  '{grep_pattern: $regex, candidate_vars: $candidate_vars}')

persist_payload_json() {
  local json_payload="$1"
  local tmp_path=""
  local mktemp_status=0

  tmp_path=$(TMPDIR="${tmp_stage_dir}" mktemp 2>/dev/null) || mktemp_status=$?
  if [[ ${mktemp_status} -ne 0 ]]; then
    mktemp_status=0
    tmp_path=$(mktemp 2>/dev/null) || mktemp_status=$?
  fi

  if [[ ${mktemp_status} -ne 0 || -z "${tmp_path}" ]]; then
    printf ''
    return 1
  fi

  if printf '%s\n' "${json_payload}" >"${tmp_path}"; then
    printf '%s' "${tmp_path}"
    return 0
  fi

  rm -f "${tmp_path}" || true
  printf ''
  return 1
}

detect_errno_from_text() {
  local text="$1"
  if [[ "${text}" == *"Operation not permitted"* ]]; then
    printf 'EPERM'
  elif [[ "${text}" == *"Permission denied"* ]]; then
    printf 'EACCES'
  else
    printf ''
  fi
}

status="partial"
errno_value=""
message="No approvals mode indicator found"
raw_exit_code=""

stdout_text=""
stderr_text=""
set +e
command_output=$(env | grep -m 1 -E "${regex}" 2>&1)
exit_code=$?
set -e

if [[ ${exit_code} -eq 0 ]]; then
  stdout_text="${command_output}"
  stderr_text=""
else
  stdout_text=""
  stderr_text="${command_output}"
fi

raw_exit_code="${exit_code}"

detected_var=""
detected_value=""

if [[ ${exit_code} -eq 0 ]]; then
  status="success"
  message="Approvals mode indicator present"
  first_line="${stdout_text%%$'\n'*}"
  detected_var="${first_line%%=*}"
  detected_value="${first_line#*=}"
elif [[ ${exit_code} -eq 1 ]]; then
  status="partial"
  message="No approvals mode env variable present"
else
  status="denied"
  errno_value=$(detect_errno_from_text "${stderr_text}")
  message="env grep for approvals mode blocked with exit code ${exit_code}"
fi

raw_payload=$(jq -n \
  --arg detected_var "${detected_var}" \
  --arg detected_value "${detected_value}" \
  --arg stdout "${stdout_text}" \
  --arg stderr "${stderr_text}" \
  --argjson candidate_vars "${candidate_vars_json}" \
  '{detected_var: ($detected_var | if length > 0 then . else null end),
    detected_value: ($detected_value | if length > 0 then . else null end),
    candidate_vars: $candidate_vars,
    stdout: $stdout,
    stderr: $stderr}')

payload_json=$(jq -n \
  --arg stdout_snippet "${stdout_text}" \
  --arg stderr_snippet "${stderr_text}" \
  --argjson raw "${raw_payload}" \
  '{stdout_snippet: ($stdout_snippet | if length > 400 then (.[:400] + "…") else . end),
    stderr_snippet: ($stderr_snippet | if length > 400 then (.[:400] + "…") else . end),
    raw: $raw}')

payload_candidate=""
if payload_candidate=$(persist_payload_json "${payload_json}"); then
  payload_file="${payload_candidate}"
else
  payload_file=""
fi

if [[ -z "${payload_file}" ]]; then
  printf 'agent_approvals_mode_env: payload persistence denied; emitting minimal record\n' >&2
fi

emit_args=(
  --run-mode "${run_mode}"
  --probe-name "${probe_name}"
  --probe-version "1"
  --primary-capability-id "${primary_capability_id}"
  --command "${command_executed}"
  --category "agent_policy"
  --verb "inspect"
  --target "env approvals mode"
  --status "${status}"
  --errno "${errno_value}"
  --message "${message}"
  --raw-exit-code "${raw_exit_code}"
  --operation-args "${operation_args}"
)

if [[ -n "${payload_file}" ]]; then
  emit_args+=(--payload-file "${payload_file}")
fi

"${emit_record_bin}" "${emit_args[@]}"
