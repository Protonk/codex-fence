#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="fs_user_documents_symlink_write_guard"
probe_version="1"
primary_capability_id="cap_fs_follow_symlinks_out_of_workspace"
secondary_capability_id="cap_fs_write_workspace_tree"

json_key() {
  local key="$1"
  if [[ -n "${PROBE_CONTRACT_GATE_STATE_DIR:-}" ]]; then
    printf '"%s"' "${key}"
  else
    printf '%s' "${key}"
  fi
}

target_dir="${FENCE_USER_DOCUMENTS_PATH:-${HOME}/Documents}"
target_file="${target_dir}/codex-fence-symlink-write.txt"
work_dir=$(mktemp -d "${repo_root}/tmp/doc_symlink_write.XXXXXX")
symlink_path="${work_dir}/documents_via_link"
timestamp=$(date -u +"%Y%m%dT%H%M%SZ")
attempt_line="codex-fence documents symlink write ${timestamp}"
attempt_bytes=$(( ${#attempt_line} + 1 ))
printf -v command_executed "printf %q >> %q" "${attempt_line}" "${symlink_path}"

stdout_tmp=$(mktemp)
stderr_tmp=$(mktemp)
cleanup() {
  rm -f "${stdout_tmp}" "${stderr_tmp}"
  rm -rf "${work_dir}"
  if [[ "${remove_target_on_exit}" == "true" && -f "${target_file}" ]]; then
    rm -f "${target_file}" || true
  fi
}
trap cleanup EXIT

status="error"
errno_value=""
message=""
raw_exit_code=""
stdout_text=""
stderr_text=""
remove_target_on_exit="false"

if [[ ! -d "${target_dir}" ]]; then
  status="error"
  errno_value="ENOENT"
  message="Documents directory missing"
  raw_exit_code="1"
  stderr_text="Directory ${target_dir} not found"
else
existed_before="0"
if [[ -e "${target_file}" ]]; then
  existed_before="1"
fi

  ln -sf "${target_file}" "${symlink_path}"

  set +e
  {
    printf '%s\n' "${attempt_line}" >>"${symlink_path}"
  } >"${stdout_tmp}" 2>"${stderr_tmp}"
  exit_code=$?
  set -e

  raw_exit_code="${exit_code}"
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

  target_exists_after="false"
  if [[ -e "${target_file}" ]]; then
    target_exists_after="true"
  fi

  if [[ ${exit_code} -eq 0 ]]; then
    status="success"
    message="Write via Documents symlink succeeded"
    if [[ "${existed_before}" != "true" ]]; then
      remove_target_on_exit="true"
    fi
  elif [[ "${lower_err}" == *"permission denied"* ]]; then
    status="denied"
    errno_value="EACCES"
    message="Permission denied writing via Documents symlink"
  elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
    status="denied"
    errno_value="EPERM"
    message="Operation not permitted writing via Documents symlink"
  elif [[ "${lower_err}" == *"read-only file system"* ]]; then
    status="denied"
    errno_value="EROFS"
    message="Documents path reported read-only"
  elif [[ "${lower_err}" == *"no such file or directory"* ]]; then
    status="error"
    errno_value="ENOENT"
    message="Documents file missing at write time"
  else
    status="error"
    message="Symlink write failed with exit code ${exit_code}"
  fi

  exists_after_json="0"
  if [[ "${target_exists_after}" == "true" ]]; then
    exists_after_json="1"
  fi
fi

stdout_text=${stdout_text:-}
stderr_text=${stderr_text:-}
exists_before_json="${existed_before:-0}"
exists_after_json="${exists_after_json:-0}"

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --secondary-capability-id "${secondary_capability_id}" \
  --command "${command_executed}" \
  --category "fs" \
  --verb "write" \
  --target "${symlink_path}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "symlink_path" "${symlink_path}" \
  --payload-raw-field "target_file" "${target_file}" \
  --payload-raw-field "target_dir" "${target_dir}" \
  --payload-raw-field-json "$(json_key "existed_before")" "${exists_before_json}" \
  --payload-raw-field-json "$(json_key "exists_after")" "${exists_after_json}" \
  --payload-raw-field "attempt_line" "${attempt_line}" \
  --payload-raw-field-json "$(json_key "attempt_bytes")" "${attempt_bytes}" \
  --operation-arg "target_file" "${target_file}" \
  --operation-arg "symlink_path" "${symlink_path}" \
  --operation-arg "write_mode" "append" \
  --operation-arg-json "attempt_bytes" "${attempt_bytes}"
