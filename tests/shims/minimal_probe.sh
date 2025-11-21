#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Minimal probe shim used by integration tests. It touches only temporary files
# and emits a deterministic boundary object so suites can assert harness output.
# -----------------------------------------------------------------------------
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)
repo_root_candidate="${script_dir}"
repo_root=""
while [[ -z "${repo_root}" && "${repo_root_candidate}" != "/" ]]; do
  # Walk upward until bin/emit-record appears—this anchors the repo root.
  if [[ -x "${repo_root_candidate}/bin/emit-record" ]]; then
    repo_root="${repo_root_candidate}"
    break
  fi
  repo_root_candidate=$(cd "${repo_root_candidate}/.." >/dev/null 2>&1 && pwd)
done
if [[ -z "${repo_root}" ]]; then
  echo "minimal_probe: unable to locate repo root" >&2
  exit 1
fi
emit_record_bin="${repo_root}/bin/emit-record"

probe_name="tests_fixture_probe"
run_mode="${FENCE_RUN_MODE:-baseline}"
primary_capability_id="cap_fs_read_workspace_tree"
workspace_tmp=$(mktemp -d)
target_file="${workspace_tmp}/fixture.txt"
trap 'rm -rf "${workspace_tmp}"' EXIT

printf 'fixture-line' > "${target_file}"
# Mirror what bin/fence-run would capture so the record looks realistic.
command_executed="printf fixture-line > ${target_file}"

payload_tmp=$(mktemp)
trap 'rm -rf "${workspace_tmp}" "${payload_tmp}"' EXIT

# Build a payload stub instead of reading from stdin so tests stay hermetic.
jq -n --arg stdout_snippet "fixture ok" --arg stderr_snippet "" --argjson raw '{"probe":"fixture"}' \
  '{stdout_snippet: $stdout_snippet, stderr_snippet: $stderr_snippet, raw: $raw}' > "${payload_tmp}"

# Emit the same boundary object a real probe would create.
"${emit_record_bin}" \
  --run-mode "${run_mode}" \
  --probe-name "${probe_name}" \
  --probe-version "1" \
  --primary-capability-id "${primary_capability_id}" \
  --command "${command_executed}" \
  --category "fs" \
  --verb "read" \
  --target "${target_file}" \
  --status success \
  --raw-exit-code 0 \
  --payload-file "${payload_tmp}" \
  --operation-args '{"fixture":true}'
