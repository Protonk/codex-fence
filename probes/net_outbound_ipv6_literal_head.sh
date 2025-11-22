#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
emit_record_bin="${repo_root}/bin/emit-record"

run_mode="${FENCE_RUN_MODE:-baseline}"
probe_name="net_outbound_ipv6_literal_head"
probe_version="1"
primary_capability_id="cap_net_outbound_any"
secondary_capability_id="cap_net_disabled_with_tag"

json_key() {
  local key="$1"
  if [[ -n "${PROBE_CONTRACT_GATE_STATE_DIR:-}" ]]; then
    printf '"%s"' "${key}"
  else
    printf '%s' "${key}"
  fi
}

curl_bin="${FENCE_CURL_BIN:-/usr/bin/curl}"
target_url="${FENCE_NET_IPV6_LITERAL_URL:-https://[2606:4700:4700::1111]/}"
max_time="${FENCE_CURL_MAX_TIME:-10}"
connect_timeout="${FENCE_CURL_CONNECT_TIMEOUT:-5}"
network_disabled_marker="${CODEX_SANDBOX_NETWORK_DISABLED:-}"

printf -v command_executed "%q -g -I --max-time %s --connect-timeout %s -sS %q" \
  "${curl_bin}" "${max_time}" "${connect_timeout}" "${target_url}"

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

if [[ ! -x "${curl_bin}" ]]; then
  status="error"
  errno_value="ENOENT"
  message="curl binary missing"
  raw_exit_code="1"
  stderr_text="curl not found at ${curl_bin}"
else
  set +e
  "${curl_bin}" -g -I --max-time "${max_time}" --connect-timeout "${connect_timeout}" -sS \
    "${target_url}" >"${stdout_tmp}" 2>"${stderr_tmp}"
  exit_code=$?
  set -e

  raw_exit_code="${exit_code}"
  stdout_text=$(tr -d '\0' <"${stdout_tmp}")
  stderr_text=$(tr -d '\0' <"${stderr_tmp}")
  lower_err=$(printf '%s' "${stderr_text}" | tr 'A-Z' 'a-z')

  status_line=$(printf '%s' "${stdout_text}" | grep -m 1 -i '^http/' || true)
  http_status=""
  if [[ -n "${status_line}" ]]; then
    http_status=$(printf '%s' "${status_line}" | awk '{print $2}')
  fi

  if [[ ${exit_code} -eq 0 ]]; then
    status="success"
    message="IPv6 literal HEAD succeeded${http_status:+ (${http_status})}"
  elif [[ ${exit_code} -eq 7 ]] || [[ "${lower_err}" == *"failed to connect"* ]] || [[ "${lower_err}" == *"couldn't connect"* ]]; then
    errno_value="ECONNREFUSED"
    if [[ -n "${network_disabled_marker}" ]]; then
      status="denied"
      message="IPv6 literal blocked; network disabled marker set"
    else
      status="partial"
      message="IPv6 literal blocked without network-disabled marker"
    fi
  elif [[ "${lower_err}" == *"could not resolve host"* ]]; then
    status="error"
    errno_value="EAI_AGAIN"
    message="curl reported DNS failure for IPv6 literal"
  elif [[ "${lower_err}" == *"network is unreachable"* ]]; then
    status="denied"
    errno_value="ENETUNREACH"
    message="Network unreachable for IPv6 literal"
  elif [[ "${lower_err}" == *"permission denied"* ]]; then
    status="denied"
    errno_value="EACCES"
    message="Permission denied for IPv6 literal request"
  elif [[ "${lower_err}" == *"operation not permitted"* ]]; then
    status="denied"
    errno_value="EPERM"
    message="Operation not permitted for IPv6 literal request"
  else
    status="error"
    message="curl IPv6 literal request failed with exit code ${exit_code}"
  fi
fi

status_line_field=(--payload-raw-null "status_line")
http_status_field=(--payload-raw-null "http_status")
if [[ -n "${status_line:-}" ]]; then
  status_line_field=(--payload-raw-field "status_line" "${status_line}")
fi
if [[ -n "${http_status:-}" ]]; then
  http_status_field=(--payload-raw-field "http_status" "${http_status}")
fi

network_disabled_field=(--payload-raw-null "network_disabled_marker")
if [[ -n "${network_disabled_marker}" ]]; then
  network_disabled_field=(--payload-raw-field "network_disabled_marker" "${network_disabled_marker}")
fi

"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "${probe_version}" \
  --primary-capability-id "${primary_capability_id}" \
  --secondary-capability-id "${secondary_capability_id}" \
  --command "${command_executed}" \
  --category "network" \
  --verb "connect" \
  --target "${target_url}" \
  --status "${status}" \
  --errno "${errno_value}" \
  --message "${message}" \
  --raw-exit-code "${raw_exit_code}" \
  --payload-stdout "${stdout_text}" \
  --payload-stderr "${stderr_text}" \
  --payload-raw-field "target_url" "${target_url}" \
  --payload-raw-field "curl_bin" "${curl_bin}" \
  --payload-raw-field "method" "HEAD" \
  "${status_line_field[@]}" \
  "${http_status_field[@]}" \
  "${network_disabled_field[@]}" \
  --payload-raw-field-json "$(json_key "max_time_seconds")" "${max_time}" \
  --payload-raw-field-json "$(json_key "connect_timeout_seconds")" "${connect_timeout}" \
  --operation-arg "target_url" "${target_url}" \
  --operation-arg "method" "HEAD" \
  --operation-arg-json "max_time_seconds" "${max_time}" \
  --operation-arg-json "connect_timeout_seconds" "${connect_timeout}" \
  --operation-arg "network_disabled_marker" "${network_disabled_marker}"
