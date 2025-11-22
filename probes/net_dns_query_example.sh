#!/usr/bin/env bash
set -euo pipefail

# why: bare DNS query isolates whether name resolution is allowed independent of HTTP/TLS egress
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="net_dns_query_example"
probe_version="1"
primary_capability_id="cap_net_outbound_any"

json_key() {
  local key="$1"
  if [[ -n "${PROBE_CONTRACT_GATE_STATE_DIR:-}" ]]; then
    printf '"%s"' "${key}"
  else
    printf '%s' "${key}"
  fi
}

target_host="${FENCE_DNS_TEST_NAME:-example.com}"
timeout_seconds="${FENCE_DNS_TIMEOUT:-3}"
if ! [[ "${timeout_seconds}" =~ ^[0-9]+$ ]]; then
  timeout_seconds=3
fi

dns_tool=""
if command -v dig >/dev/null 2>&1; then
  dns_tool="dig"
elif command -v nslookup >/dev/null 2>&1; then
  dns_tool="nslookup"
fi

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
stdout_text=""
stderr_text=""
network_disabled_marker="${CODEX_SANDBOX_NETWORK_DISABLED:-}"

if [[ -z "${dns_tool}" ]]; then
  status="error"
  errno_value="ENOENT"
  message="dig/nslookup not available"
  raw_exit_code="1"
  printf -v command_executed "(missing dns tool)"
else
  if [[ "${dns_tool}" == "dig" ]]; then
    printf -v command_executed "%q +time=%q +tries=1 +short %q" "${dns_tool}" "${timeout_seconds}" "${target_host}"
    set +e
    dig +time="${timeout_seconds}" +tries=1 +short "${target_host}" >"${stdout_tmp}" 2>"${stderr_tmp}"
    exit_code=$?
    set -e
  else
    # nslookup lacks a direct timeout flag; rely on default resolver timeout
    printf -v command_executed "%q %q" "${dns_tool}" "${target_host}"
    set +e
    nslookup "${target_host}" >"${stdout_tmp}" 2>"${stderr_tmp}"
    exit_code=$?
    set -e
  fi

  raw_exit_code="${exit_code}"
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

  has_answer="false"
  if [[ -n "${stdout_text}" ]]; then
    has_answer="true"
  fi

  if [[ ${exit_code} -eq 0 && "${has_answer}" == "true" ]]; then
    status="success"
    message="DNS query returned data"
  else
    # classify common denial/error signals
    if [[ "${lower_err}" == *"connection timed out"* ]]; then
      status="denied"
      errno_value="ETIMEDOUT"
      message="DNS timed out"
    elif [[ "${lower_err}" == *"no servers could be reached"* ]]; then
      status="denied"
      errno_value="ENETUNREACH"
      message="No DNS servers reachable"
    elif [[ "${lower_err}" == *"permission denied"* ]]; then
      status="denied"
      errno_value="EACCES"
      message="DNS query permission denied"
    elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
      status="denied"
      errno_value="EPERM"
      message="DNS query operation not permitted"
    elif [[ -n "${network_disabled_marker}" ]]; then
      status="denied"
      errno_value="ENETUNREACH"
      message="Network disabled marker present"
    else
      status="error"
      message="DNS query failed with exit code ${exit_code}"
    fi
  fi
fi

answer_snippet=""
if [[ -n "${stdout_text}" ]]; then
  answer_snippet=$(printf '%s' "${stdout_text}" | head -n 5)
fi

network_env_field=(--payload-raw-null "network_disabled_env")
if [[ -n "${network_disabled_marker}" ]]; then
  network_env_field=(--payload-raw-field "network_disabled_env" "${network_disabled_marker}")
fi

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "net" \
  --verb "dns_lookup" \
  --target "${target_host}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "dns_tool" "${dns_tool:-missing}" \
  --payload-raw-field "target_host" "${target_host}" \
  --payload-raw-field "answer_snippet" "${answer_snippet}" \
  --payload-raw-field-json "$(json_key "timeout_seconds")" "${timeout_seconds}" \
  "${network_env_field[@]}" \
  --operation-arg "dns_tool" "${dns_tool:-missing}" \
  --operation-arg "host" "${target_host}" \
  --operation-arg-json "timeout_seconds" "${timeout_seconds}"
