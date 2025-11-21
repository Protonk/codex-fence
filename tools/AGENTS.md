# Tools Playbook for Agents

This directory hosts helpers for automated agents developing in the repo. 

## Available tooling

- `capabilities_adapter.sh`: fast, simple reader for `capabilities.json`.
- `contract_gate/`: Static and dynamic checker used by Probe Authors when creating new probes.
- `pathtools.sh`: canonicalizes probe paths and exports the `resolve_probe_script_path`
  + `portable_realpath` helpers shared by the contract tools.
- `audits/INTERPRETERS.md`: AI agent prompts for audits.

## Modfiying tooling
Before changing or adding tooling:
- Mirror the existing safety posture: every script sets `set -euo pipefail`,
  resolves `repo_root`, and fails fast if prerequisites are absent.
- Ship hermetic behaviors. Store helper jq/awk/sed snippets inline (as the
  adapter does) so contributors can audit the script without hunting external
  files.
- Validate inputs early and emit actionable errors (include file paths the way
   the current tools do).
- Document your intent at the top of the script with a guard-rail summary so
  future agents understand the blast radius and know which invariants the tool
  defends.
