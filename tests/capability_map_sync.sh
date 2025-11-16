#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)
# shellcheck source=tests/lib/utils.sh
source "${script_dir}/lib/utils.sh"

cd "${REPO_ROOT}"

status=0

echo "capability_map_sync: validating capability metadata"

mapfile -t capability_ids < <(awk '$1=="-" && $2=="id:" {print $3}' spec/capabilities.yaml)
if [[ ${#capability_ids[@]} -eq 0 ]]; then
  echo "capability_map_sync: no capability IDs found in spec/capabilities.yaml" >&2
  exit 1
fi

declare -A capability_set=()
for cap in "${capability_ids[@]}"; do
  capability_set["${cap}"]=1
done

declare -A coverage_caps=()
declare -A coverage_has_probe=()
declare -A coverage_probe_lists=()

while IFS=$'\t' read -r cap_id has_probe probe_list; do
  coverage_caps["${cap_id}"]=1
  coverage_has_probe["${cap_id}"]="${has_probe}"
  coverage_probe_lists["${cap_id}"]="${probe_list}"
  if [[ -z "${capability_set["${cap_id}"]:-}" ]]; then
    echo "  [FAIL] capability_map_sync: coverage references unknown capability '${cap_id}'" >&2
    status=1
  fi
done < <(jq -r 'to_entries[] | [.key, (.value.has_probe|tostring), (.value.probe_ids|join(","))] | @tsv' spec/capabilities-coverage.json)

for cap in "${capability_ids[@]}"; do
  if [[ -z "${coverage_caps["${cap}"]:-}" ]]; then
    echo "  [FAIL] capability_map_sync: spec/capabilities-coverage.json missing entry for '${cap}'" >&2
    status=1
  fi
done

shopt -s nullglob
probe_scripts=(probes/*.sh)
if [[ ${#probe_scripts[@]} -eq 0 ]]; then
  echo "capability_map_sync: no probes found" >&2
  exit 1
fi

declare -A cap_to_probes=()
declare -A probe_to_cap=()
declare -A known_probes=()

for script in "${probe_scripts[@]}"; do
  probe_name=$(extract_probe_var "${script}" "probe_name" || true)
  primary_cap=$(extract_probe_var "${script}" "primary_capability_id" || true)

  if [[ -z "${probe_name}" ]]; then
    echo "  [FAIL] capability_map_sync: ${script} is missing probe_name" >&2
    status=1
    continue
  fi
  known_probes["${probe_name}"]=1

  if [[ -z "${primary_cap}" ]]; then
    echo "  [FAIL] capability_map_sync: ${script} is missing primary_capability_id" >&2
    status=1
    continue
  fi

  probe_to_cap["${probe_name}"]="${primary_cap}"
  if [[ -z "${cap_to_probes["${primary_cap}"]:-}" ]]; then
    cap_to_probes["${primary_cap}"]="${probe_name}"
  else
    cap_to_probes["${primary_cap}"]+=" ${probe_name}"
  fi

  if [[ -z "${capability_set["${primary_cap}"]:-}" ]]; then
    echo "  [FAIL] capability_map_sync: ${script} references unknown capability '${primary_cap}'" >&2
    status=1
  fi

done

in_array() {
  local needle="$1"
  shift
  for value in "$@"; do
    if [[ "${value}" == "${needle}" ]]; then
      return 0
    fi
  done
  return 1
}

for cap_id in "${!coverage_caps[@]}"; do
  has_probe_flag=${coverage_has_probe["${cap_id}"]}
  coverage_value=${coverage_probe_lists["${cap_id}"]}
  coverage_array=()
  if [[ -n "${coverage_value}" ]]; then
    IFS=',' read -r -a coverage_array <<< "${coverage_value}"
  fi

  actual_value=${cap_to_probes["${cap_id}"]:-}
  actual_array=()
  if [[ -n "${actual_value}" ]]; then
    read -r -a actual_array <<< "${actual_value}"
  fi

  if [[ "${has_probe_flag}" == "true" && ${#actual_array[@]} -eq 0 ]]; then
    echo "  [FAIL] capability_map_sync: ${cap_id} marked has_probe=true but no probes declare it" >&2
    status=1
  fi

  if [[ "${has_probe_flag}" == "false" && ${#actual_array[@]} -gt 0 ]]; then
    echo "  [FAIL] capability_map_sync: ${cap_id} marked has_probe=false but probes ${actual_array[*]} target it" >&2
    status=1
  fi

  for listed_probe in "${coverage_array[@]}"; do
    if [[ -z "${listed_probe}" ]]; then
      continue
    fi
    if [[ -z "${known_probes["${listed_probe}"]:-}" ]]; then
      echo "  [FAIL] capability_map_sync: ${cap_id} lists unknown probe '${listed_probe}'" >&2
      status=1
      continue
    fi
    if [[ "${probe_to_cap["${listed_probe}"]}" != "${cap_id}" ]]; then
      echo "  [FAIL] capability_map_sync: ${listed_probe} in coverage for ${cap_id} but script targets ${probe_to_cap["${listed_probe}"]}" >&2
      status=1
    fi
  done

  for actual_probe in "${actual_array[@]}"; do
    if ! in_array "${actual_probe}" "${coverage_array[@]}"; then
      echo "  [FAIL] capability_map_sync: ${cap_id} missing probe '${actual_probe}' in coverage list" >&2
      status=1
    fi
  done

done

if [[ ${status} -ne 0 ]]; then
  exit ${status}
fi

echo "capability_map_sync: PASS"
