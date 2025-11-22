#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="proc_exec_tmp_script"
probe_version="1"
primary_capability_id="cap_proc_exec_toolchain_outside_workspace"

json_key() {
  local key="$1"
  if [[ -n "${PROBE_CONTRACT_GATE_STATE_DIR:-}" ]]; then
    printf '"%s"' "${key}"
  else
    printf '%s' "${key}"
  fi
}

tmp_root=$(mktemp -d "/tmp/codex-fence-tmp-exec.XXXXXX" 2>/dev/null || mktemp -d "${repo_root}/tmp/tmp-exec.XXXXXX")
script_path="${tmp_root}/tmp_exec_probe.sh"
expected_line="tmp script executed"
printf -v command_executed "%q" "${script_path}"

cat <<'EOS' >"${script_path}"
#!/usr/bin/env bash
set -euo pipefail
echo "tmp script executed"
EOS
chmod +x "${script_path}" 2>/dev/null || true

stdout_tmp=$(mktemp)
stderr_tmp=$(mktemp)
cleanup() {
  rm -f "${stdout_tmp}" "${stderr_tmp}"
  rm -rf "${tmp_root}"
}
trap cleanup EXIT

status="error"
errno_value=""
message=""
raw_exit_code=""

set +e
"${script_path}" >"${stdout_tmp}" 2>"${stderr_tmp}"
exit_code=$?
set -e

raw_exit_code="${exit_code}"
stdout_text=$(tr -d '\0' <"${stdout_tmp}")
stderr_text=$(tr -d '\0' <"${stderr_tmp}")
lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

stdout_trimmed=$(printf '%s' "${stdout_text}" | head -n 5)
matched="false"
if [[ "${stdout_text}" == *"${expected_line}"* ]]; then
  matched="true"
fi

if [[ ${exit_code} -eq 0 && "${matched}" == "true" ]]; then
  status="success"
  message="Temporary script executed outside workspace"
elif [[ ${exit_code} -eq 0 ]]; then
  status="partial"
  message="Temporary script ran but output mismatch"
elif [[ ${exit_code} -eq 126 ]] || [[ "${lower_err}" == *"permission denied"* ]]; then
  status="denied"
  errno_value="EACCES"
  message="Execution denied for temporary script"
elif [[ ${exit_code} -eq 127 ]] || [[ "${lower_err}" == *"no such file"* ]]; then
  status="error"
  errno_value="ENOENT"
  message="Temporary script missing"
elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
  status="denied"
  errno_value="EPERM"
  message="Operation not permitted executing temporary script"
else
  status="error"
  message="Temporary script failed with exit code ${exit_code}"
fi

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "proc" \
  --verb "exec" \
  --target "${script_path}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "script_path" "${script_path}" \
  --payload-raw-field "tmp_root" "${tmp_root}" \
  --payload-raw-field "expected_line" "${expected_line}" \
  --payload-raw-field "stdout_snippet" "${stdout_trimmed}" \
  --payload-raw-field-json "$(json_key "matched_expected_output")" "${matched}" \
  --operation-arg "script_path" "${script_path}" \
  --operation-arg "location" "/tmp" \
  --operation-arg "invocation" "direct_exec" \
  --operation-arg "expected_line" "${expected_line}"
