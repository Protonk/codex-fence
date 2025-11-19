#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Ensures bin/fence-run refuses to execute scripts outside probes/ even when
# passed absolute paths or symlinks that try to escape the workspace.
# -----------------------------------------------------------------------------
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)

source "${script_dir}/../library/utils.sh"

cd "${REPO_ROOT}"

echo "probe_resolution_guards: verifying probe resolution denies escapes"

outside_script=$(mktemp)
cat <<'SCRIPT' > "${outside_script}"
#!/usr/bin/env bash
echo "should never run"
exit 0
SCRIPT
chmod +x "${outside_script}"

symlink_probe="${REPO_ROOT}/probes/tests_probe_resolution_symlink.sh"
if [[ -e "${symlink_probe}" ]]; then
  echo "probe_resolution_guards: fixture already exists at ${symlink_probe}" >&2
  exit 1
fi

cleanup() {
  rm -f "${outside_script}" "${symlink_probe}"
}
trap cleanup EXIT

if bin/fence-run baseline "${outside_script}" >/dev/null 2>&1; then
  echo "probe_resolution_guards: fence-run executed script outside probes/ (absolute path)" >&2
  exit 1
fi

ln -s "${outside_script}" "${symlink_probe}"

if bin/fence-run baseline "tests_probe_resolution_symlink" >/dev/null 2>&1; then
  echo "probe_resolution_guards: fence-run followed a symlink that pointed outside probes/" >&2
  exit 1
fi

echo "probe_resolution_guards: PASS"
