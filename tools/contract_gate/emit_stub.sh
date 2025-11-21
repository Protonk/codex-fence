#!/usr/bin/env bash
set -euo pipefail

# This stub validates the runtime emit-record contract for the probe contract gate.

state_dir=${PROBE_CONTRACT_GATE_STATE_DIR:-}
if [[ -z "${state_dir}" ]]; then
  echo "emit-record stub: PROBE_CONTRACT_GATE_STATE_DIR is not set" >&2
  exit 1
fi

mkdir -p "${state_dir}" >/dev/null 2>&1

counter_file="${state_dir}/emit_record_invocations"
error_file="${state_dir}/emit_record_errors.log"
status_file="${state_dir}/emit_record_status"
args_file="${state_dir}/emit_record_last_args"

fail() {
  local message="$1"
  printf '%s\n' "${message}" >>"${error_file}"
  printf '%s\n' "emit-record stub: ${message}" >&2
  exit 1
}

record_invocation() {
  local current="0"
  if [[ -f "${counter_file}" ]]; then
    current=$(cat "${counter_file}" 2>/dev/null || printf '0')
  fi
  if ! [[ "${current}" =~ ^[0-9]+$ ]]; then
    current="0"
  fi
  current=$((current + 1))
  printf '%s' "${current}" >"${counter_file}"
}

record_invocation

printf '%s\0' "$@" >"${args_file}" 2>/dev/null || true

required_run_mode=""
required_probe_name=""
required_primary_capability_id=""
required_run_mode=${PROBE_CONTRACT_EXPECTED_RUN_MODE:-}
required_probe_name=${PROBE_CONTRACT_EXPECTED_PROBE_NAME:-}
required_primary_capability_id=${PROBE_CONTRACT_EXPECTED_PRIMARY_CAPABILITY_ID:-}
capabilities_json=${PROBE_CONTRACT_CAPABILITIES_JSON:-}
capabilities_adapter=${PROBE_CONTRACT_CAPABILITIES_ADAPTER:-}

ensure_jq() {
  if ! command -v jq >/dev/null 2>&1; then
    fail "jq is not available for JSON validation"
  fi
}

ensure_jq

run_mode=""
probe_name=""
probe_version=""
primary_capability_id=""
command_value=""
category=""
verb=""
target=""
status_value=""
errno_value=""
message_value=""
payload_file=""
operation_args=""
raw_exit_code=""

run_mode_set="false"
probe_name_set="false"
probe_version_set="false"
primary_capability_id_set="false"
command_set="false"
category_set="false"
verb_set="false"
target_set="false"
status_set="false"
payload_file_set="false"
operation_args_set="false"
raw_exit_code_set="false"

assign_flag() {
  local var_name="$1"
  local set_flag_name="$2"
  local human_name="$3"
  local value="$4"
  local allow_empty="$5"

  if [[ "${!set_flag_name-}" == "true" ]]; then
    fail "${human_name} provided multiple times"
  fi
  if [[ "${allow_empty}" != "allow-empty" && -z "${value}" ]]; then
    fail "${human_name} cannot be empty"
  fi
  printf -v "${var_name}" '%s' "${value}"
  printf -v "${set_flag_name}" '%s' "true"
}

secondary_capability_ids=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --run-mode)
      [[ $# -ge 2 ]] || fail "--run-mode requires a value"
      assign_flag run_mode run_mode_set "--run-mode" "$2" "no-empty"
      shift 2
      ;;
    --probe-name)
      [[ $# -ge 2 ]] || fail "--probe-name requires a value"
      assign_flag probe_name probe_name_set "--probe-name" "$2" "no-empty"
      shift 2
      ;;
    --probe-version)
      [[ $# -ge 2 ]] || fail "--probe-version requires a value"
      assign_flag probe_version probe_version_set "--probe-version" "$2" "no-empty"
      shift 2
      ;;
    --primary-capability-id)
      [[ $# -ge 2 ]] || fail "--primary-capability-id requires a value"
      assign_flag primary_capability_id primary_capability_id_set "--primary-capability-id" "$2" "no-empty"
      shift 2
      ;;
    --secondary-capability-id)
      [[ $# -ge 2 ]] || fail "--secondary-capability-id requires a value"
      secondary_capability_ids+=("$2")
      shift 2
      ;;
    --command)
      [[ $# -ge 2 ]] || fail "--command requires a value"
      assign_flag command_value command_set "--command" "$2" "no-empty"
      shift 2
      ;;
    --category)
      [[ $# -ge 2 ]] || fail "--category requires a value"
      assign_flag category category_set "--category" "$2" "no-empty"
      shift 2
      ;;
    --verb)
      [[ $# -ge 2 ]] || fail "--verb requires a value"
      assign_flag verb verb_set "--verb" "$2" "no-empty"
      shift 2
      ;;
    --target)
      [[ $# -ge 2 ]] || fail "--target requires a value"
      assign_flag target target_set "--target" "$2" "no-empty"
      shift 2
      ;;
    --status)
      [[ $# -ge 2 ]] || fail "--status requires a value"
      assign_flag status_value status_set "--status" "$2" "no-empty"
      shift 2
      ;;
    --errno)
      [[ $# -ge 2 ]] || fail "--errno requires a value"
      errno_value="$2"
      shift 2
      ;;
    --message)
      [[ $# -ge 2 ]] || fail "--message requires a value"
      message_value="$2"
      shift 2
      ;;
    --payload-file)
      [[ $# -ge 2 ]] || fail "--payload-file requires a path"
      assign_flag payload_file payload_file_set "--payload-file" "$2" "no-empty"
      shift 2
      ;;
    --operation-args)
      [[ $# -ge 2 ]] || fail "--operation-args requires JSON"
      assign_flag operation_args operation_args_set "--operation-args" "$2" "no-empty"
      shift 2
      ;;
    --raw-exit-code)
      [[ $# -ge 2 ]] || fail "--raw-exit-code requires a value"
      assign_flag raw_exit_code raw_exit_code_set "--raw-exit-code" "$2" "no-empty"
      shift 2
      ;;
    --*)
      fail "unknown flag '$1'"
      ;;
    *)
      fail "unexpected positional argument '$1'"
      ;;
  esac
done

require_flag_value() {
  local value="$1"
  local human_name="$2"
  if [[ -z "${value}" ]]; then
    fail "${human_name} is required"
  fi
}

require_flag_value "${run_mode}" "--run-mode"
require_flag_value "${probe_name}" "--probe-name"
require_flag_value "${probe_version}" "--probe-version"
require_flag_value "${primary_capability_id}" "--primary-capability-id"
require_flag_value "${command_value}" "--command"
require_flag_value "${category}" "--category"
require_flag_value "${verb}" "--verb"
require_flag_value "${target}" "--target"
require_flag_value "${status_value}" "--status"
require_flag_value "${payload_file}" "--payload-file"
require_flag_value "${operation_args}" "--operation-args"
require_flag_value "${raw_exit_code}" "--raw-exit-code"

if [[ -n "${required_run_mode}" && "${run_mode}" != "${required_run_mode}" ]]; then
  fail "--run-mode '${run_mode}' does not match expected '${required_run_mode}'"
fi

if [[ -n "${required_probe_name}" && "${probe_name}" != "${required_probe_name}" ]]; then
  fail "--probe-name '${probe_name}' does not match expected '${required_probe_name}'"
fi

if [[ -n "${required_primary_capability_id}" && "${primary_capability_id}" != "${required_primary_capability_id}" ]]; then
  fail "--primary-capability-id '${primary_capability_id}' does not match expected '${required_primary_capability_id}'"
fi

case "${status_value}" in
  success|denied|partial|error)
    ;;
  *)
    fail "--status '${status_value}' is not in the allowed set"
    ;;
esac

if ! [[ "${raw_exit_code}" =~ ^-?[0-9]+$ ]]; then
  fail "--raw-exit-code '${raw_exit_code}' is not an integer"
fi

if [[ ! -f "${payload_file}" ]]; then
  fail "payload file '${payload_file}' does not exist"
fi

payload_size=$(wc -c <"${payload_file}" 2>/dev/null || printf '0')
if ! [[ "${payload_size}" =~ ^[0-9]+$ ]]; then
  payload_size=0
fi
max_payload_bytes=$((1024 * 1024))
if (( payload_size > max_payload_bytes )); then
  fail "payload file is larger than ${max_payload_bytes} bytes"
fi

if ! jq -e 'type == "object" and has("stdout_snippet") and has("stderr_snippet") and has("raw") and (.stdout_snippet | type == "string") and (.stderr_snippet | type == "string") and (.raw | type == "object")' "${payload_file}" >/dev/null; then
  fail "payload JSON missing required fields"
fi

if ! printf '%s' "${operation_args}" | jq -e 'type == "object"' >/dev/null 2>&1; then
  fail "--operation-args must be a JSON object"
fi

if [[ -n "${capabilities_json}" && -n "${capabilities_adapter}" && -x "${capabilities_adapter}" && -f "${capabilities_json}" ]]; then
  if ! capability_map=$("${capabilities_adapter}" "${capabilities_json}" 2>/dev/null); then
    fail "capability catalog validation failed"
  fi
  if ! printf '%s' "${capability_map}" | jq -e --arg id "${primary_capability_id}" 'has($id)' >/dev/null 2>&1; then
    fail "unknown primary_capability_id '${primary_capability_id}'"
  fi
fi

printf 'ok\n' >"${status_file}"

printf '{"probe_contract_gate":"validated"}\n'

exit 0
