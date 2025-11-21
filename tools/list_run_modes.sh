#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Shared run-mode definitions for bash helpers.
#
# Goals:
# - Keep contract gate mode coverage aligned with the modes the harness cares
#   about (baseline, codex-sandbox, codex-full).
# - Allow overrides via PROBE_CONTRACT_MODES for tests or rapid experiments.
# -----------------------------------------------------------------------------
set -euo pipefail

codex_fence_run_modes() {
  local modes=("baseline")
  if command -v codex >/dev/null 2>&1; then
    modes+=("codex-sandbox" "codex-full")
  fi
  printf '%s\n' "${modes[@]}"
}

contract_gate_modes() {
  local override="${PROBE_CONTRACT_MODES:-}"
  if [[ -n "${override}" ]]; then
    printf '%s' "${override}" | tr ',' ' ' | tr -s ' ' '\n' | sed '/^[[:space:]]*$/d'
    return 0
  fi
  codex_fence_run_modes
}
