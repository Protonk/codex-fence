## Remediation Summary

- Preflight/TMPDIR now respects the resolved workspace: `src/bin/fence_run.rs` picks tmp dirs from the workspace override (no silent fallback to repo tmp), emits a preflight denial when tmpdir setup fails, and stops using guessed capability/version data by resolving probe metadata via `ProbeMetadata` with required primary capability.
- Preflight records now carry authoritative probe metadata and use the workspace-scoped tmpdir; fallback defaults were removed, and codex preflight is skipped only after emitting a denial record that references the attempted tmp path (`src/bin/fence_run.rs`).
- Boundary objects now canonicalize `workspace_root` before emitting, avoiding relative/symlink noise (`src/bin/emit_record.rs`).
- Probe metadata parsing tracks `probe_version`, feeding the preflight path (`src/probe_metadata.rs`).
- Added unit coverage for tmpdir planning, override errors, and metadata resolution to keep the new behaviors in place (`src/bin/fence_run.rs` tests).

## What changed and why

- **Workspace-aware TMPDIR and preflight** (`src/bin/fence_run.rs`):
  - TMPDIR selection now derives from the effective workspace (CLI/env override) and only falls back to repo tmp when no override exists. This enforces the workspace-boundary promise and ensures codex preflight tests the same roots probes will use.
  - If tmpdir creation fails for all candidates, `fence-run` emits a preflight denial record (category `preflight`) instead of silently skipping codex modes. This keeps matrix runs producing the documented denial signal when sandbox writes are blocked.

- **Authoritative preflight metadata** (`src/bin/fence_run.rs`, `src/probe_metadata.rs`):
  - Preflight record emission now uses parsed probe metadata (id, version, primary_capability_id) rather than scraping single lines with fallbacks. This prevents mis-attribution when probes set metadata dynamically or non-linearly.
  - `ProbeMetadata` now captures `probe_version`, so preflight and future consumers can reflect probe versions accurately.

- **Canonical workspace root emission** (`src/bin/emit_record.rs`):
  - `emit-record` now canonicalizes the chosen workspace root (env/git/PWD/current) before writing boundary objects, reducing noise and making cross-host diffs reliable.

- **Tests** (`src/bin/fence_run.rs` tests):
  - Added coverage to assert TMPDIR respects overrides, records errors when candidates fail, and that metadata resolution prefers script values. These guard rails prevent regression of the new behaviors.
