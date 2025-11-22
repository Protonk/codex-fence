#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="fs_ssh_dir_metadata_read"
probe_version="1"
primary_capability_id="cap_fs_read_user_content"

target_dir="${FENCE_SSH_DIR:-${HOME}/.ssh}"
printf -v command_executed "ls -ld %q" "${target_dir}"

stdout_tmp=$(mktemp)
stderr_tmp=$(mktemp)
cleanup() {
  rm -f "${stdout_tmp}" "${stderr_tmp}"
}
trap cleanup EXIT

status="error"
errno_value=""
message=""
raw_exit_code=""

if [[ ! -d "${target_dir}" ]]; then
  status="error"
  errno_value="ENOENT"
  message="SSH directory missing"
  raw_exit_code="1"
  stdout_text=""
  stderr_text="Directory ${target_dir} not found"
else
  set +e
  ls -ld "${target_dir}" >"${stdout_tmp}" 2>"${stderr_tmp}"
  exit_code=$?
  set -e

  raw_exit_code="${exit_code}"
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

  if [[ ${exit_code} -eq 0 ]]; then
    status="success"
    message="Listed SSH directory metadata"
  elif [[ "${lower_err}" == *"permission denied"* ]]; then
    status="denied"
    errno_value="EACCES"
    message="Permission denied reading SSH directory"
  elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
    status="denied"
    errno_value="EPERM"
    message="Operation not permitted reading SSH directory"
  else
    status="error"
    message="ls failed with exit code ${exit_code}"
  fi
fi

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "fs" \
  --verb "read" \
  --target "${target_dir}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text:-}" \
  --payload-stderr "${stderr_text:-}" \
  --payload-raw-field "target_dir" "${target_dir}" \
  --operation-arg "path_type" "directory" \
  --operation-arg "read_type" "list"
