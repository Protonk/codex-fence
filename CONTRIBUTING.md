# General Contributions

Thanks for improving codex-fence! This document covers repository-level work:
tests, helper libraries, tooling, docs, and automation. For probe-specific
expectations see the Probe Author contract in [probes/AGENTS.md](probes/AGENTS.md)--human and AI agents writing new probes should mainly concern themselves with that. 

## Principles

- **Portability first.** Probes must not introduce spurious signals due to inconsistencies between platforms. Organize and write helper functions to support consistent harness behavior on e.g. macOS or the `codex-universal` container.
- **Single responsibility.** Helpers stay pure and composable; tooling avoids reaching into unrelated directories unless required.
- **Document contracts.** When adding configuration fields, schema changes, or
  helper functions, update the relevant documentation in the same change.
- **Comment code.** Code comments are first class documentation objects, easing auditing and bug finding.

## Helpers and tooling

- The probe contract gate lives at `tools/validate_contract_gate.sh`.
  Keep it lightweight so single-probe loops (`--probe <id>` or `make probe`)
  remain instant.
- `bin/emit-record`, `bin/fence-run`, and any new helpers must avoid
  introducing runtime dependencies beyond Bash and the Rust standard
  library—keep probe plumbing lightweight and portable.
- After touching Rust helpers under `src/bin/`, run `make build-bin` (or
  `tools/sync_bin_helpers.sh`) so the synced binaries in `bin/` match
  your changes; probes, docs, and tests assume `bin/<helper>` resolves them
  directly.

## Tests

- The Rust-based guard rails live in `tests/suite.rs` and run via
  `cargo test --test suite` (`boundary_object_schema`, `harness_smoke_probe_fixture`, `baseline_no_codex_smoke`, etc.). When expanding coverage, keep these tests
  hermetic and deterministic.
- Place reusable mocks under `tests/mocks/` and keep them synced
  with reality.
- Add new guard rails to `tests/suite.rs` when the checks are global or
  slow. Ensure they short-circuit quickly on missing prerequisites so macOS
  authors can still iterate with `cargo test`.

# Keep schema documentation in sync

Updates to the capabilities catalog, located at `schema/capabilities.json`, or the boundary object schema (`schema/boundary_object.json`) require matching updates in their documentation:
- `schema/capabilities.json` is documented in `docs/capabilities.md`
- `schema/boundary_object.json` is documented in `docs/boundary_object.md`
Ensure these files stay in sync.
