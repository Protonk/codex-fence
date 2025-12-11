# Probe Contract and Boundary Object (cfbo-v1)

`probe` records every probe run as a versioned JSON “boundary object”. Version **cfbo-v1** is the current contract. It incorporates the current capability catalog (via the Rust capability index backed by the catalog schema and bundled catalog) so every record carries a snapshot of the capability metadata it referenced.

Each boundary object captures *one* probe execution in one run mode. Probes are tiny scripts stored under `probes/<probe_id>.sh` (filenames match the capability catalog’s probe ids) that:

1. Use `#!/usr/bin/env bash` with `set -euo pipefail`.
2. Perform exactly one observable action (write a file, open a socket, read `sysctl`, etc.).
3. Collect the stdout/stderr snippets needed to describe that action plus any structured payload.
4. Call `bin/emit-record` once with `--run-mode "$FENCE_RUN_MODE"` plus the metadata described below.
5. Exit with status `0` after emitting JSON. They must not print anything else to stdout; use stderr only for debugging.

See `probes/AGENTS.md` for the workflow details expected from probe authors.

## Formal commitments

The project commits to the cfbo-v1 contract as specified by:

- The machine-readable JSON schema at `schema/boundary_object.json`.
- This document’s field-by-field explanations.

Within cfbo-v1, the required fields, field names, and semantics described below are stable. Changes that break compatibility (renaming fields, relaxing/adding required fields, or altering meanings) require creating a new schema version and updating this document to match.

## Boundary object layout (cfbo-v1)

The machine-readable definition lives in `schema/boundary_object.json` and is enforced by `bin/emit-record`.

| Field | Required | Description |
| --- | --- | --- |
| `schema_version` | yes | Always `"cfbo-v1"`. |
| `capabilities_schema_version` | yes | The catalog key from the loaded capability catalog. It is a string with no whitespace such as `macOS_codex_v1`. |
| `stack` | yes | Fingerprint of the external CLI (when present) and OS stack that hosted the probe. |
| `probe` | yes | Identity and capability linkage for the probe implementation. |
| `run` | yes | Execution metadata for this invocation (mode, workspace, command). This harness intentionally omits timestamps so records stay stateless. |
| `operation` | yes | Description of the sandbox-facing operation being attempted. |
| `result` | yes | Normalized observed outcome plus error metadata. |
| `payload` | yes | Small probe-specific diagnostics and structured raw data. |
| `capability_context` | yes | Snapshot of the primary/secondary capability entries as seen through the capability index. |

### `stack`

Populated automatically by `bin/detect-stack`.

| Field | Required | Meaning |
| --- | --- | --- |
| `external_cli_version` | yes (nullable) | Output of the configured external CLI `--version` if available, else `null`. |
| `external_profile` | yes (nullable) | Runner profile/mode if known (`FENCE_EXTERNAL_PROFILE` or `FENCE_CODEX_PROFILE`). |
| `sandbox_mode` | yes (nullable) | `read-only`, `workspace-write`, `danger-full-access`, or `null` for baseline runs. |
| `os` | yes | Value from `uname -srm`. |

### `probe`

`bin/emit-record` validates capability IDs by loading the bundled capability catalog directly (the legacy adapter remains for automation).

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Stable slug (usually the probe filename) such as `fs_outside_workspace`. |
| `version` | yes | Probe-local semantic/string version; bump when behavior changes. |
| `primary_capability_id` | yes | Capability tested by this probe. Must match the capability catalog. |
| `secondary_capability_ids` | yes | Zero or more supporting capability ids (unique, may be empty). |



### `run`

cfbo-v1 deliberately does **not** capture timestamps or run durations. The harness stays stateless; downstream consumers that need clocks or diffing data must add it outside the boundary object.

| Field | Required | Meaning |
| --- | --- | --- |
| `mode` | yes | `baseline`, `codex-sandbox`, or `codex-full`; matches `bin/probe-exec`. |
| `workspace_root` | yes (nullable) | Canonical workspace root exported by `bin/probe-exec` (`FENCE_WORKSPACE_ROOT`), falling back to `git rev-parse` / `pwd` if unset. |
| `command` | yes | Human/machine-usable string describing the actual command. |


### `operation`

Describes the resource being touched.

| Field | Required | Meaning |
| --- | --- | --- |
| `category` | yes | High-level domain: `fs`, `net`, `proc`, `sysctl`, `agent_policy`, etc. |
| `verb` | yes | `read`, `write`, `exec`, `connect`, ... depending on the probe. |
| `target` | yes | Path/host/syscall/descriptor being addressed. |
| `args` | yes | Free-form JSON object with structured flags (modes, sizes, offsets). Use `{}` if unused. |

### `result`

Normalized observation of what happened, regardless of how the probe implemented it.

| Field | Required | Meaning |
| --- | --- | --- |
| `observed_result` | yes | One of `success`, `denied`, `partial`, `error`. |
| `raw_exit_code` | yes (nullable) | Exit code from the command that performed the operation. |
| `errno` | yes (nullable) | Errno mnemonic (`EACCES`, `EPERM`, ...) if inferred. |
| `message` | yes (nullable) | Short human summary of the outcome. |
| `error_detail` | yes (nullable) | Additional diagnostics for unexpected failures. |

Interpretation of `observed_result`:

- `success`: the sandbox allowed the operation outright.
- `denied`: explicitly blocked by sandbox/policy (permission denied, EPERM, etc.).
- `partial`: some sub-step succeeded while another failed; note details in `message` / `payload.raw`.
- `error`: probe failed for reasons unrelated to sandbox policy (implementation bug, transient infra error).

cfbo-v1 does not carry runtime durations. The probe contract stays clock-free; any per-probe timings shown by development tooling are diagnostics for authors and do not reach the boundary object.

### `payload`

Catch-all for probe-specific breadcrumbs. Keep these small (<4 KB).

| Field | Required | Meaning |
| --- | --- | --- |
| `stdout_snippet` | yes (nullable) | Up to ~400 characters of stdout (truncated if needed). |
| `stderr_snippet` | yes (nullable) | Same for stderr. |
| `raw` | yes | Structured JSON object for any other data (timings, file stats, HTTP responses). |

### `capability_context`

Every record includes the capability snapshot(s) that were resolved when the probe was emitted. This lets downstream tooling trace exactly which schema version and metadata were in effect.

| Field | Required | Meaning |
| --- | --- | --- |
| `primary` | yes | Object with `id`, `category`, `layer` from the capability index. |
| `secondary` | no | Array of the same structure (may be empty). |

## Example

A trimmed record from `probes/fs_outside_workspace.sh` (writes outside the workspace and expects a denial):

```json
{
  "schema_version": "cfbo-v1",
  "capabilities_schema_version": "macOS_codex_v1",
  "probe": {
    "id": "fs_outside_workspace",
    "version": "1",
    "primary_capability_id": "cap_fs_write_workspace_tree",
    "secondary_capability_ids": []
  },
  "run": {
    "mode": "codex-sandbox",
    "workspace_root": "/Users/example/project",
    "command": "printf 'probe write ...' >> '/tmp/probe-outside-root-test'"
  },
  "operation": {
    "category": "fs",
    "verb": "write",
    "target": "/tmp/probe-outside-root-test",
    "args": {"write_mode": "append", "attempt_bytes": 38}
  },
  "result": {
    "observed_result": "denied",
    "raw_exit_code": 1,
    "errno": "EACCES",
    "message": "Permission denied",
    "error_detail": null
  },
  "payload": {
    "stdout_snippet": "",
    "stderr_snippet": "bash: /tmp/probe-outside-root-test: Permission denied",
    "raw": {}
  },
  "capability_context": {
    "primary": {
      "id": "cap_fs_write_workspace_tree",
      "category": "filesystem",
      "layer": "os_sandbox"
    },
    "secondary": []
  },
  "stack": {
    "external_cli_version": "codex 1.2.3",
    "external_profile": "Auto",
    "sandbox_mode": "workspace-write",
    "os": "Darwin 23.3.0 arm64"
  }
}
```

## Updating the commitments

When the boundary-object contract needs to change in a backward-incompatible way, follow this procedure:

1. Add a new schema file (for example `schema/boundary_object_cfbo_v2.json`) with an updated `$id`, `title`, and `schema_version` constant while preserving the prior file unchanged.
2. Update this document to describe the new version, including any added or removed fields and the rationale for the change.
3. Refresh `AGENTS.md`, `README.md`, `docs/probes.md`, and any tooling that validates or emits boundary objects (`bin/emit-record`, `tests/`, probe helpers) so they reference and enforce the new schema.
4. Document the migration expectations (whether older versions are still accepted, and for how long) alongside the new version announcement.
5. Use `--boundary-schema` (or `FENCE_BOUNDARY_SCHEMA_PATH`) to validate or emit against a drop-in schema file when experimenting with new versions; the active schema’s `schema_version` will be written into emitted records.

Until such a change is made, cfbo-v1 remains the committed contract.
