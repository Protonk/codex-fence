#!/usr/bin/env bash
# probe_runner_api: module
set -euo pipefail

probe_name="tests_runner_module_probe"
probe_version="1"
primary_capability_id="cap_fs_read_workspace_tree"

run_probe() {
  local repo_root
  repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." >/dev/null 2>&1 && pwd)
  local target_file="${repo_root}/README.md"
  local command_executed="head -n1 ${target_file}"
  local payload_tmp
  payload_tmp=$(mktemp)
  trap '[[ -n "${payload_tmp:-}" ]] && rm -f "${payload_tmp}"' EXIT

  local stdout_snippet
  stdout_snippet=$(head -n1 "${target_file}")

  jq -n \
    --arg stdout "${stdout_snippet}" \
    --arg stderr "" \
    --argjson raw '{"runner_fixture":true}' \
    '{stdout_snippet: $stdout, stderr_snippet: $stderr, raw: $raw}' > "${payload_tmp}"

  emit_result \
    --status success \
    --command "${command_executed}" \
    --category "fs" \
    --verb "read" \
    --target "${target_file}" \
    --raw-exit-code 0 \
    --message "Runner fixture read README.md" \
    --payload-file "${payload_tmp}" \
    --operation-args '{"fixture":true}'
}
