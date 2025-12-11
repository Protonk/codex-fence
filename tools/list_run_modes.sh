#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Shared run-mode definitions for bash helpers.
#
# Goals:
# - Keep contract gate mode coverage aligned with the modes the harness cares
#   about (baseline, codex-full, codex-sandbox).
# - Allow overrides via PROBE_CONTRACT_MODES for tests or rapid experiments.
# -----------------------------------------------------------------------------
set -euo pipefail

external_cli_run_modes() {
  local modes=("baseline" "codex-full")
  local external_cli="${FENCE_EXTERNAL_CLI:-${CODEX_CLI:-codex}}"
  if command -v "${external_cli}" >/dev/null 2>&1; then
    modes+=("codex-sandbox")
  fi
  printf '%s\n' "${modes[@]}"
}

contract_gate_modes() {
  local override="${PROBE_CONTRACT_MODES:-}"
  if [[ -n "${override}" ]]; then
    printf '%s' "${override}" | tr ',' ' ' | tr -s ' ' '\n' | sed '/^[[:space:]]*$/d'
    return 0
  fi
  external_cli_run_modes
}
