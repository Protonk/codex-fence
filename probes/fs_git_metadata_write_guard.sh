#!/usr/bin/env bash
set -euo pipefail

# Targets cap_fs_read_git_metadata: verifies writes inside .git are blocked even though reads remain allowed.
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"
portable_path_helper="${repo_root}/bin/portable-path"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="fs_git_metadata_write_guard"
probe_version="1"
primary_capability_id="cap_fs_read_git_metadata"
target_path="${repo_root}/.git/codex-fence-write-test"
attempt_line="codex-fence git-metadata-write $(date -u +%Y-%m-%dT%H:%M:%SZ)"
attempt_bytes=$(( ${#attempt_line} + 1 ))
printf -v command_executed "printf %q >> %q" "${attempt_line}" "${target_path}"

stdout_tmp=$(mktemp)
stderr_tmp=$(mktemp)
remove_target_on_exit="false"
target_exists_before="false"
target_exists_after="false"
target_exists_final="false"
target_size_before=""
target_size_after=""
target_realpath=""
cleanup() {
  rm -f "${stdout_tmp}" "${stderr_tmp}"
  if [[ "${remove_target_on_exit}" == "true" && -f "${target_path}" && "${target_exists_before}" != "true" ]]; then
    rm -f "${target_path}"
  fi
}
trap cleanup EXIT

if [[ -x "${portable_path_helper}" ]]; then
  target_realpath=$("${portable_path_helper}" realpath "${target_path}" 2>/dev/null || printf '')
fi

if [[ -e "${target_path}" ]]; then
  target_exists_before="true"
  target_size_before=$(stat -f%z "${target_path}" 2>/dev/null || printf '')
fi

status="error"
errno_value=""
message=""
raw_exit_code=""

set +e
bash -c 'printf "%s\n" "$1" >> "$2"' _ "${attempt_line}" "${target_path}" \
  >"${stdout_tmp}" 2>"${stderr_tmp}"
exit_code=$?
set -e
raw_exit_code="${exit_code}"
stderr_text=$(tr -d '\0' <"${stderr_tmp}")
stdout_text=$(tr -d '\0' <"${stdout_tmp}")

if [[ -e "${target_path}" ]]; then
  target_exists_after="true"
  target_size_after=$(stat -f%z "${target_path}" 2>/dev/null || printf '')
fi

if [[ ${exit_code} -eq 0 ]]; then
  if [[ "${target_exists_after}" == "true" ]]; then
    status="success"
    message="Write inside .git succeeded"
    remove_target_on_exit="true"
  else
    status="partial"
    errno_value="ENOENT"
    message="Write reported success but .git target missing"
  fi
else
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')
  if [[ "${lower_err}" == *"permission denied"* ]]; then
    status="denied"
    errno_value="EACCES"
    message="Permission denied writing .git"
  elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
    status="denied"
    errno_value="EPERM"
    message="Operation not permitted"
  elif [[ "${lower_err}" == *"read-only file system"* ]]; then
    status="denied"
    errno_value="EROFS"
    message="Read-only file system"
  elif [[ "${lower_err}" == *"no such file or directory"* ]]; then
    status="error"
    errno_value="ENOENT"
    message=".git directory not found"
  else
    status="error"
    message="Write failed with exit code ${exit_code}"
  fi
fi

target_exists_final="${target_exists_after}"
if [[ "${remove_target_on_exit}" == "true" && "${target_exists_after}" == "true" && "${target_exists_before}" != "true" ]]; then
  if rm -f "${target_path}" 2>/dev/null; then
    target_exists_final="false"
  elif [[ -e "${target_path}" ]]; then
    target_exists_final="true"
  fi
fi

size_flag=(--payload-raw-null "resulting_size")
if [[ -n "${target_size_after}" ]]; then
  size_flag=(--payload-raw-field "resulting_size" "${target_size_after}")
fi
size_before_flag=(--payload-raw-null "size_before")
if [[ -n "${target_size_before}" ]]; then
  size_before_flag=(--payload-raw-field "size_before" "${target_size_before}")
fi

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "fs" \
  --verb "write" \
  --target "${target_path}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "target_path" "${target_path}" \
  --payload-raw-field "target_realpath" "${target_realpath}" \
  --payload-raw-field "attempt_bytes" "${attempt_bytes}" \
  --payload-raw-field "existed_before" "${target_exists_before}" \
  --payload-raw-field "exists_after" "${target_exists_after}" \
  --payload-raw-field "exists_after_cleanup" "${target_exists_final}" \
  "${size_before_flag[@]}" \
  --operation-arg "write_mode" "append" \
  --operation-arg "path_context" ".git" \
  --operation-arg-json "attempt_bytes" "${attempt_bytes}" \
  "${size_flag[@]}"
