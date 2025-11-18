#!/usr/bin/env bash
#
# Helper functions for probe-runner module API probes. Scripts executed via
# bin/probe-runner source this file so they can call emit_result to hand their
# observations back to the runner.

set -euo pipefail

: "${PROBE_RUNNER_RESULT_FILE:?probe_runner_module: PROBE_RUNNER_RESULT_FILE is not set}"

probe_runner_emitted=0

emit_result() {
  local status=""
  local command=""
  local category=""
  local verb=""
  local target=""
  local raw_exit_code=""
  local errno_value=""
  local message=""
  local duration_ms=""
  local error_detail=""
  local payload_file=""
  local operation_args='{}'

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --status) status="$2"; shift 2 ;;
      --command) command="$2"; shift 2 ;;
      --category) category="$2"; shift 2 ;;
      --verb) verb="$2"; shift 2 ;;
      --target) target="$2"; shift 2 ;;
      --raw-exit-code) raw_exit_code="$2"; shift 2 ;;
      --errno) errno_value="$2"; shift 2 ;;
      --message) message="$2"; shift 2 ;;
      --duration-ms) duration_ms="$2"; shift 2 ;;
      --error-detail) error_detail="$2"; shift 2 ;;
      --payload-file) payload_file="$2"; shift 2 ;;
      --operation-args) operation_args="$2"; shift 2 ;;
      *)
        echo "emit_result: unknown flag $1" >&2
        return 1
        ;;
    esac
  done

  if [[ -z "${status}" || -z "${command}" || -z "${category}" || -z "${verb}" || -z "${target}" ]]; then
    echo "emit_result: missing required flags" >&2
    return 1
  fi

  if [[ ${probe_runner_emitted} -ne 0 ]]; then
    echo "emit_result: multiple emissions detected" >&2
    return 1
  fi

  if [[ -z "${operation_args}" ]]; then
    operation_args='{}'
  fi

  jq -n \
    --arg status "${status}" \
    --arg command "${command}" \
    --arg category "${category}" \
    --arg verb "${verb}" \
    --arg target "${target}" \
    --arg raw_exit_code "${raw_exit_code}" \
    --arg errno_value "${errno_value}" \
    --arg message "${message}" \
    --arg duration_ms "${duration_ms}" \
    --arg error_detail "${error_detail}" \
    --arg payload_file "${payload_file}" \
    --argjson operation_args "${operation_args}" \
    '{
      status: $status,
      command: $command,
      category: $category,
      verb: $verb,
      target: $target,
      raw_exit_code: (($raw_exit_code | select(length > 0)) // null),
      errno: (($errno_value | select(length > 0)) // null),
      message: (($message | select(length > 0)) // null),
      duration_ms: (($duration_ms | select(length > 0)) // null),
      error_detail: (($error_detail | select(length > 0)) // null),
      payload_file: (($payload_file | select(length > 0)) // null),
      operation_args: $operation_args
    }' > "${PROBE_RUNNER_RESULT_FILE}"

  probe_runner_emitted=1
}

probe_runner_main() {
  probe_runner_emitted=0
  if ! declare -f run_probe >/dev/null 2>&1; then
    echo "probe_runner_module: run_probe function is not defined" >&2
    return 1
  fi
  run_probe
  if [[ ${probe_runner_emitted} -eq 0 ]]; then
    echo "probe_runner_module: run_probe did not call emit_result" >&2
    return 1
  fi
}
