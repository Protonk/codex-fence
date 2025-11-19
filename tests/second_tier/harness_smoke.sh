#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Basic end-to-end test for bin/fence-run baseline mode. It runs a known fixture
# probe and asserts the recorded boundary object contains fixture markers.
# -----------------------------------------------------------------------------
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)

source "${script_dir}/../library/utils.sh"

cd "${REPO_ROOT}"

fixture_name="tests_fixture_probe"
probe_path="probes/${fixture_name}.sh"
fixture_source="tests/library/fixtures/probe_fixture.sh"

if [[ -e "${probe_path}" ]]; then
  echo "harness_smoke: fixture probe already exists at ${probe_path}" >&2
  exit 1
fi

cp "${fixture_source}" "${probe_path}"
chmod +x "${probe_path}"
trap 'rm -f "${probe_path}"' EXIT

output_tmp=$(mktemp)
trap 'rm -f "${probe_path}" "${output_tmp}"' EXIT

bin/fence-run baseline "${fixture_name}" > "${output_tmp}"

jq -e --arg expected_workspace "${REPO_ROOT}" '
  .probe.id == "tests_fixture_probe" and
  .operation.category == "fs" and
  .result.observed_result == "success" and
  (.payload.raw.probe == "fixture") and
  (.run.workspace_root == $expected_workspace)
' "${output_tmp}" >/dev/null
# The jq filter encodes the "happy path" contract the harness should honor.

echo "harness_smoke: PASS"
