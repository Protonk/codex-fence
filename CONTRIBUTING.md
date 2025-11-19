# General Contributions

Thanks for improving codex-fence! This document covers repository-level work:
tests, helper libraries, tooling, docs, and automation. For probe-specific
expectations see the Probe Author contract in `AGENTS.md`--human and AI agents writing new probes should mainly concern themselves with that. 

## Scope

Use this guide when you plan to:
- Edit or add shell/Ruby helpers under `tools/` or `bin/`.
- Modify the Makefile, capability catalog, schema, or adapters.
- Extend `tests/` or its fixtures.
- Update documentation outside a single probe (README, `docs/*.md`, etc.).
Following this guide keeps the repo coherent for both human and AI probe
authors while preserving the portability guarantees that make `codex-fence`
valuable.

## Principles

- **Portability first.** Probes must not introduce spurious signals due to inconsistencies between platforms. Organize and write helper functions to support consistent harness behavior on e.g. macOS or the `codex-universal` container.
- **Single responsibility.** Helpers stay pure and composable; probes remain
  small; tooling avoids reaching into unrelated directories unless required.
- **Document contracts.** When adding configuration fields, schema changes, or
  helper functions, update the relevant Markdown (`docs/probes.md`,
  `docs/boundary_object.md`, `spec/AGENTS.md`, or README) in the same change.

## Repository areas

### Helpers and tooling

- Probe helpers live under `lib/` (one function per script, e.g.,
  `lib/portable_realpath.sh`). Keep helpers pure (no global state or side
  effects) so probes and tests can source them safely.
- Project-level scripts (lint, validation, adapters) live under `tools/`.
  `tools/light_lint.sh` is the shared lint entry point—prefer extending it for
  new checks instead of duplicating logic elsewhere.
- `bin/emit-record`, `bin/fence-run`, and any new binaries must stay
  dependency-free beyond POSIX + `jq`.

### Tests

- `tests/run.sh` orchestrates two tiers: a fast lint/static-contract pass and
  the second-tier suites (`capability_map_sync`, `boundary_object_schema`,
  `harness_smoke`, `baseline_no_codex_smoke`). When expanding coverage, keep
  the fast tier lightweight so `tests/run.sh --probe <id>` remains instant.
- Place reusable fixtures under `tests/library/fixtures/` and keep them synced
  with the capability catalog (the validation scripts scan these files too).
- Add new suites under `tests/second_tier/` when the checks are global or slow.
  Ensure they short-circuit quickly on missing prerequisites so macOS authors
  can still run `make test`.

### Documentation and catalogs

- Changing `spec/capabilities.json` or `docs/capabilities_coverage.json`
  requires matching updates to `spec/AGENTS.md` plus any references in
  README/AGENTS.
- Updates to the boundary-object schema (`schema/boundary_object_cfbo_v1.json`)
  must be mirrored in `docs/boundary_object.md` and, if the authoring workflow
  changes, in `docs/probes.md`.
- If you add run modes, helper commands, or workflow changes, reflect them in
  README (usage/tests sections) and `AGENTS.md`.
