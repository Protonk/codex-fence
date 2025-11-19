#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Validates that bin/fence-run can skip exporting FENCE_WORKSPACE_ROOT so the
# emitted boundary object falls back to git/pwd for workspace detection.
# -----------------------------------------------------------------------------
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)

source "${script_dir}/../library/utils.sh"

cd "${REPO_ROOT}"

fixture_name="tests_fixture_probe"
probe_path="probes/${fixture_name}.sh"
fixture_source="tests/library/fixtures/probe_fixture.sh"

if [[ -e "${probe_path}" ]]; then
  echo "workspace_root_fallback: fixture probe already exists at ${probe_path}" >&2
  exit 1
fi

cp "${fixture_source}" "${probe_path}"
chmod +x "${probe_path}"

tmp_run_dir=$(mktemp -d)
output_tmp=$(mktemp)

cleanup() {
  rm -f "${probe_path}"
  rm -rf "${tmp_run_dir}"
  rm -f "${output_tmp}"
}
trap cleanup EXIT

(
  cd "${tmp_run_dir}"
  FENCE_WORKSPACE_ROOT="" "${REPO_ROOT}/bin/fence-run" baseline "${fixture_name}" > "${output_tmp}"
)

jq -e --arg expected_workspace "${tmp_run_dir}" '
  .probe.id == "tests_fixture_probe" and
  .run.workspace_root == $expected_workspace
' "${output_tmp}" >/dev/null

echo "workspace_root_fallback: PASS"
