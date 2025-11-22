#!/usr/bin/env bash
set -euo pipefail

# why: deletion confirms workspace write permission includes unlink, not just create/append
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="fs_workspace_delete_file"
probe_version="1"
primary_capability_id="cap_fs_write_workspace_tree"

json_key() {
  local key="$1"
  if [[ -n "${PROBE_CONTRACT_GATE_STATE_DIR:-}" ]]; then
    printf '"%s"' "${key}"
  else
    printf '%s' "${key}"
  fi
}

attempt_line="codex-fence delete $(date -u +%Y-%m-%dT%H:%M:%SZ) $$"
target_path=$(mktemp "${repo_root}/.codex-fence-delete.XXXXXX")
printf -v create_command "printf %q > %q" "${attempt_line}" "${target_path}"
printf -v command_executed "rm %q" "${target_path}"

stdout_tmp=$(mktemp)
stderr_tmp=$(mktemp)
cleanup() {
  rm -f "${stdout_tmp}" "${stderr_tmp}"
  # Best-effort cleanup if delete failed
  if [[ -f "${target_path}" ]]; then
    rm -f "${target_path}" || true
  fi
}
trap cleanup EXIT

status="error"
errno_value=""
message=""
raw_exit_code=""
exists_before_json="false"
exists_after_json="false"

# create file inside workspace to delete
set +e
bash -c 'printf "%s\n" "$1" > "$2"' _ "${attempt_line}" "${target_path}" 2>"${stderr_tmp}" >"${stdout_tmp}"
create_exit=$?
set -e

if [[ ${create_exit} -ne 0 ]]; then
  raw_exit_code="${create_exit}"
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')
  if [[ "${lower_err}" == *"permission denied"* ]]; then
    status="denied"
    errno_value="EACCES"
    message="Permission denied creating workspace file"
  elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
    status="denied"
    errno_value="EPERM"
    message="Operation not permitted creating workspace file"
  else
    status="error"
    message="Failed to create workspace file for delete"
  fi
else
  stdout_text=""
  stderr_text=""
  :
  exists_before_json="true"
fi

if [[ ${create_exit} -eq 0 ]]; then
  set +e
  rm "${target_path}" >"${stdout_tmp}" 2>"${stderr_tmp}"
  exit_code=$?
  set -e
  raw_exit_code="${exit_code}"
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

  exists_after="false"
  if [[ -e "${target_path}" ]]; then
    exists_after="true"
    exists_after_json="true"
  fi

  if [[ ${exit_code} -eq 0 && "${exists_after}" == "false" ]]; then
    status="success"
    message="Deleted workspace temp file"
  elif [[ ${exit_code} -eq 0 ]]; then
    status="partial"
    message="rm exited 0 but file still present"
  else
    if [[ "${lower_err}" == *"permission denied"* ]]; then
      status="denied"
      errno_value="EACCES"
      message="Permission denied deleting workspace file"
    elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
      status="denied"
      errno_value="EPERM"
      message="Operation not permitted deleting workspace file"
    elif [[ "${lower_err}" == *"no such file or directory"* ]]; then
      status="error"
      errno_value="ENOENT"
      message="Target missing at delete time"
    else
      status="error"
      message="rm failed with exit code ${exit_code}"
    fi
  fi
fi

written_bytes=${#attempt_line}

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "fs" \
  --verb "delete" \
  --target "${target_path}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "target_path" "${target_path}" \
  --payload-raw-field-json "$(json_key "exists_before")" "${exists_before_json}" \
  --payload-raw-field-json "$(json_key "exists_after")" "${exists_after_json}" \
  --payload-raw-field-json "$(json_key "written_bytes")" "${written_bytes}" \
  --operation-arg "delete_target" "${target_path}" \
  --operation-arg "write_mode" "create_then_delete" \
  --operation-arg-json "written_bytes" "${written_bytes}"
