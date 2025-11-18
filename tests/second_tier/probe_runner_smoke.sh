#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)
# shellcheck source=tests/library/utils.sh
source "${script_dir}/../library/utils.sh"

cd "${REPO_ROOT}"

fixture_script="tests/library/fixtures/probe_runner_module_probe.sh"

if [[ ! -f "${fixture_script}" ]]; then
  echo "probe_runner_smoke: missing fixture at ${fixture_script}" >&2
  exit 1
fi

output_tmp=$(mktemp)
trap 'rm -f "${output_tmp}"' EXIT

bin/probe-runner --mode baseline --script "${fixture_script}" > "${output_tmp}"

jq -e '
  .probe.id == "tests_runner_module_probe" and
  .result.observed_result == "success" and
  .payload.raw.runner_fixture == true
' "${output_tmp}" >/dev/null

echo "probe_runner_smoke: PASS"
