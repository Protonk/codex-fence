#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="agent_sandbox_env_marker"
primary_capability_id="cap_agent_sandbox_env_marker"
marker_var="CODEX_SANDBOX_ENV_VAR"
printf -v command_executed "printenv %s" "${marker_var}"

tmp_stage_dir="${repo_root}/tmp/${probe_name}"
mkdir -p "${tmp_stage_dir}" 2>/dev/null || true

payload_file=""

cleanup() {
  if [[ -n "${payload_file}" ]]; then
    rm -f "${payload_file}" || true
  fi
}
trap cleanup EXIT

persist_payload_json() {
  local json_payload="$1"
  local candidates=()

  if [[ -d "${tmp_stage_dir}" && -w "${tmp_stage_dir}" ]]; then
    candidates+=("${tmp_stage_dir}/payload.XXXXXX")
  fi
  if [[ -n "${TMPDIR:-}" ]]; then
    candidates+=("${TMPDIR%/}/agent_sandbox_env_marker_payload.XXXXXX")
  fi
  candidates+=("/tmp/agent_sandbox_env_marker_payload.XXXXXX")

  local template=""
  for template in "${candidates[@]}"; do
    local tmp_path=""
    if tmp_path=$(mktemp "${template}" 2>/dev/null); then
      if printf '%s\n' "${json_payload}" >"${tmp_path}"; then
        printf '%s' "${tmp_path}"
        return 0
      fi
      rm -f "${tmp_path}" || true
    fi
  done

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

operation_args=$(jq -n --arg marker_var "${marker_var}" '{marker_var: $marker_var}')

stdout_text=""
stderr_text=""
set +e
command_output=$(printenv "${marker_var}" 2>&1)
exit_code=$?
set -e

if [[ ${exit_code} -eq 0 ]]; then
  stdout_text="${command_output}"
else
  stderr_text="${command_output}"
fi

raw_exit_code="${exit_code}"
marker_value=$(printf '%s' "${stdout_text}" | tr -d '\n')
fallback_marker="${CODEX_SANDBOX:-}"

status="partial"
errno_value=""
message="Sandbox env marker absent"

if [[ ${exit_code} -eq 0 ]]; then
  status="success"
  message="${marker_var}=${marker_value}"
elif [[ ${exit_code} -eq 1 ]]; then
  status="partial"
  if [[ -n "${fallback_marker}" ]]; then
    message="${marker_var} unset; CODEX_SANDBOX=${fallback_marker} present"
  else
    message="Sandbox env marker absent"
  fi
else
  errno_value=$(detect_errno_from_text "${stderr_text}")
  if [[ -n "${errno_value}" ]]; then
    status="denied"
  else
    status="error"
  fi
  message="printenv exited with ${exit_code}"
fi

raw_payload=$(jq -n \
  --arg marker_var "${marker_var}" \
  --arg marker_value "${marker_value}" \
  --arg fallback_marker "${fallback_marker}" \
  --arg stdout "${stdout_text}" \
  --arg stderr "${stderr_text}" \
  '{marker_var: $marker_var,
    marker_value: ($marker_value | if length > 0 then . else null end),
    fallback_marker: ($fallback_marker | if length > 0 then . else null end),
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
fi

emit_args=(
  --run-mode "${run_mode}"
  --probe-name "${probe_name}"
  --probe-version "1"
  --primary-capability-id "${primary_capability_id}"
  --command "${command_executed}"
  --category "agent_policy"
  --verb "inspect"
  --target "${marker_var}"
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
