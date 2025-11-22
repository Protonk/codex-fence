## Topline Summary

Adversarial summary: Capability/catalog validation and probe contract basics are locked in by emit-record + CapabilityIndex checks and the static/dynamic gate (tools/validate_contract_gate.sh, tests/suite.rs fixture tests). Workspace isolation and preflight promises are aspirational: tempdir selection ignores overrides, preflight records can carry wrong capability metadata, and no tests cover these codepaths, so small changes to argument parsing or tmpdir creation could silently erode the fence signals. 

- Workspace overrides don’t drive temp/preflight locations: `fence-run` always builds `workspace_tmpdir` from the repo root and sets `TMPDIR` from it (`src/bin/fence_run.rs:27`, `src/bin/fence_run.rs:34`). If the caller passes `--workspace-root` or clears `FENCE_WORKSPACE_ROOT`, probes and codex preflight still write under the repo `tmp/` rather than the requested workspace, undermining the workspace-boundary promise and preflight accuracy.
- Codex preflight records can be mis-attributed: capability and version are scraped with a one-line matcher and fall back to `cap_fs_read_workspace_tree`/`1` on failure (`src/bin/fence_run.rs:380`, `src/bin/fence_run.rs:424`, `src/bin/fence_run.rs:426`). Any probe that sets metadata dynamically or on a later line will emit a denial record tagged to the wrong capability, breaking the catalog→record linkage the docs promise.
- Preflight only runs when the repo-root `tmp/` is writable; otherwise codex modes skip the preflight path entirely (`src/bin/fence_run.rs:41`, `src/bin/fence_run.rs:490`). On hosts where `tmp/` creation fails (common when using workspace overrides or read-only checkouts), codex modes will just fail instead of emitting the documented `observed_result=denied` preflight record, so matrix runs lose the “keep going with a denial record” guarantee.
- Workspace roots aren’t canonicalized when `emit-record` falls back to `FENCE_WORKSPACE_ROOT`/`git`/`PWD` (`src/bin/emit_record.rs:694`). That can embed relative or symlinked paths in boundary objects, making cross-host comparisons noisy despite the portability/canonicalization guidance.
- Guard rails don’t cover the codex preflight path: `tests/suite.rs` exercises baseline and fixture flows but never asserts TMPDIR/workspace override behavior or the shape of preflight records, so the above regressions can land unnoticed.

## Detailed Findings

### Workspace override ignored for temp/preflight targets
- Evidence: `workspace_tmpdir` is always derived from the canonicalized repo root (`src/bin/fence_run.rs:27-36`, `src/bin/fence_run.rs:207-216`) and exported as `TMPDIR` unconditionally. The resolved workspace (CLI/env override) is stored in `WorkspacePlan` but never consulted when choosing the tmpdir.
- Risk: Probes that are supposed to exercise a caller-specified workspace instead write under the repo. Codex preflight also tests the wrong path, so a host that blocks writes to the requested workspace won’t be reported.
- Fix sketch: Build `workspace_tmpdir` from the effective workspace (or skip TMPDIR export when the override is blank). Add a failure path that emits the preflight denial when tmpdir creation fails after applying overrides.

### Preflight capability attribution can drift
- Evidence: `emit_preflight_record` scrapes `primary_capability_id` and `probe_version` by scanning for a single-line assignment and falls back to hardcoded defaults on miss (`src/bin/fence_run.rs:380-400`, `src/bin/fence_run.rs:424-427`). Dynamic assignments, late definitions, or probes that template metadata will emit denial records tied to the wrong capability/version.
- Risk: Consumers diffing outputs may see capability drift or missing coverage, and remediation work could target the wrong capability.
- Fix sketch: Pass resolved probe metadata from `resolve_probe` (or a parser shared with `probe_metadata.rs`) into `emit_preflight_record` instead of guessing. Fail hard if metadata cannot be determined rather than defaulting.

### Preflight skipped when repo tmp is unwritable
- Evidence: Codex preflight is gated on `workspace_tmpdir` creation (`src/bin/fence_run.rs:41-54`, `src/bin/fence_run.rs:490-515`). If `tmp/` under the repo cannot be created, codex modes proceed without emitting the promised preflight denial, and the subsequent run fails with a generic error.
- Risk: Matrix runs lose the “continue with a denial record” guarantee; failures look like harness errors instead of policy denials, masking host constraints.
- Fix sketch: Attempt tmpdir creation relative to the effective workspace, fall back carefully, and if all attempts fail, emit a preflight denial that records the tmpdir failure instead of skipping.

### Workspace root not canonicalized in emitted records
- Evidence: `resolve_workspace_root` returns env/git/PWD/current without canonicalization (`src/bin/emit_record.rs:694-729`). `fence-run` canonicalizes the repo-root default, but any override or git/PWD path can remain relative or symlinked in the boundary object.
- Risk: Cross-host diffs and downstream analysis get noisy, and path-based policies may be misinterpreted.
- Fix sketch: Canonicalize the chosen workspace root inside `emit-record` (when possible) before emitting.

### Missing guard rails for codex preflight/TMPDIR semantics
- Evidence: `tests/suite.rs` covers schema validation, probe resolution, baseline/codex presence, and json-extract, but has no assertions around TMPDIR/export behavior, workspace overrides, or the preflight denial flow. Contract gate stub only checks emit-record usage, not preflight.
- Risk: The regressions above can ship undetected; portability promises around workspace isolation and preflight continuity are unenforced.
- Fix sketch: Add integration tests that run `fence-run` with workspace overrides and unwritable repo tmp to assert TMPDIR selection and preflight record emission. Consider unit tests around `emit_preflight_record` metadata selection.

## Overall Assurance
Strong points:
- Capability catalog and boundary-object validation are enforced by `CapabilityIndex`/`emit-record` and schema-backed Rust tests (`tests/suite.rs:1-171`).
- Probe contract basics (shebang, strict mode, single emit) are enforced by `tools/validate_contract_gate.sh` and the dynamic gate/fixtures.

Weak points:
- Workspace isolation and preflight semantics are only partially implemented and lack tests.
- Preflight metadata extraction relies on brittle scraping and defaults, undermining catalog alignment.
- Canonicalization of emitted workspace paths is inconsistent.

Priority remediation items:
1) Align tmpdir/preflight with workspace overrides and emit denials when tmpdir setup fails.
2) Feed authoritative probe metadata into preflight emission; drop silent defaults.
3) Canonicalize workspace_root in `emit-record`.
4) Add tests for preflight/TMPDIR/workspace override behavior and denial record shape.
