# Fencerunner

> Run small, explicit probes against a sandbox or runtime and capture what actually happened as structured JSON.

Fencerunner is infrastructure. It does not impose a particular sandbox or policy;
instead, it gives you a way to **describe capabilities**, **exercise them with tiny
shell probes**, and **record the results as versioned JSON “boundary objects”**
that can be analyzed later.

The top‑level CLI is called `probe`. It discovers probes, runs them in
well‑defined modes, validates their outputs against schemas and capability
catalogs, and keeps the contract between “what probes promise” and “what
actually ran” tight.

For contributor‑focused details, see `CONTRIBUTING.md`. For contract‑level
guidance, start with the AGENTS files.

## Mental model

At a high level, Fencerunner is built from three ideas:

- **Probes** — small Bash scripts under `probes/<probe_id>.sh`. Each performs
  exactly one observable action (for example, “write a file outside the
  workspace”) and calls a helper binary to emit a single JSON record describing
  what happened.
- **Capability catalog** — a JSON catalog that names the behaviors you care
  about (`cap_fs_write_workspace_tree`, `cap_net_connect_loopback`, …) and
  explains how they map onto low‑level operations. Probes refer to capabilities
  by id; the harness resolves those ids against the catalog.
- **Boundary objects (cfbo‑v1)** — the JSON records emitted by each probe run.
  They encode the catalog key, probe identity, run mode, operation details,
  normalized outcome, payload, and capability context, all validated against a
  schema.

## Core CLI surface

The primary entry point is the `probe` binary (synced into `bin/probe`).

- `probe --matrix`  
  Run a matrix of probes across one or more run modes and stream every
  boundary object as NDJSON. This is the usual way to “take a reading” of a
  sandbox or runtime.

- `probe --target`  
  Run a focused subset of probes, selected either by probe id or by capability
  id in the catalog. This is useful when you only care about a specific slice
  of behavior.

- `probe --listen`  
  Read cfbo‑v1 NDJSON (for example, from `probe --matrix`) on stdin and print a
  human‑readable summary. This is a text‑only viewer; it never changes the
  underlying JSON.


## Probes: how you measure a sandbox

Probes are intentionally boring:

- They are Bash scripts in `probes/<probe_id>.sh`.
- They use `#!/usr/bin/env bash` plus `set -euo pipefail`.
- They perform one focused operation.
- They call `bin/emit-record` exactly once to emit a JSON boundary object.
- They write nothing else to stdout (stderr is reserved for minimal diagnostics).

Each probe declares:

- a `probe.id` (the filename),
- a `primary_capability_id` and optional `secondary_capability_ids` from the
  catalog, and
- a normalized `observed_result` (`success`, `denied`, `partial`, `error`)
  plus payload snippets that capture what actually happened.

The probe author contract, examples, and test‑backed rules live in
`probes/AGENTS.md`. Start there if you are writing or modifying probes.

## Catalogs and boundary schemas

Two JSON schemas define how data flows through Fencerunner:

- **Capability catalog schema**  
  `schema/capability_catalog.schema.json` describes the shape of capability
  catalogs. The bundled catalog instance lives in
  `catalogs/macos_codex_v1.json` and is keyed by `catalog.key` (the
  `capabilities_schema_version` echoed into boundary objects).

- **Boundary object schema (cfbo‑v1)**  
  `schema/boundary_object.json` describes the cfbo‑v1 JSON records emitted by
  probes. `docs/boundary_object.md` walks each field and explains evolution
  rules.

The harness always requires a catalog and a boundary schema, but you can swap
them out without changing code:

- Use `--catalog <path>` or `FENCE_CATALOG_PATH` to point helpers at a different
  catalog file.
- Use `FENCE_ALLOWED_CATALOG_SCHEMAS` (comma‑separated) to temporarily accept
  additional catalog `schema_version` values while experimenting.
- Use `--boundary-schema <path>` or `FENCE_BOUNDARY_SCHEMA_PATH` to point
  helpers at an alternate boundary‑object schema file; the loaded
  `schema_version` is written into emitted records.

The Rust layer (`src/catalog`, `src/boundary`) validates catalogs and boundary
objects at load and emit time, and the integration tests under `tests/suite.rs`
assert that the schemas, helpers, and sample data stay in sync.

For a narrative view of these contracts, see:

- `docs/capabilities.md`
- `docs/boundary_object.md`
- `docs/probes.md`

---

## Running and developing Fencerunner

Prerequisites:

- A recent Rust toolchain (see `Cargo.toml` for the minimum version).
- A POSIX shell environment with `/bin/bash` and common Unix tools.

Build the helpers into `bin/`:

```sh
make build
```

Run the main test suite:

```sh
make test          # rebuild helpers, then cargo test --test suite
```

Common workflows:

- **Run the full probe matrix with the bundled catalog and schema**

  ```sh
  bin/probe --matrix
  ```

- **Inspect results in a human‑readable form**

  ```sh
  bin/probe --matrix | bin/probe --listen
  ```

- **Run a single probe by id**

  ```sh
  bin/probe --target --probe fs_outside_workspace --mode baseline
  ```

- **Gate a probe while authoring**

  ```sh
  tools/validate_contract_gate.sh --probe fs_outside_workspace
  # or
  bin/probe-contract-gate fs_outside_workspace
  ```

When you change Rust code under `src/` or `src/bin/`, rebuild helpers with
`make build` and re‑run `make test` to keep `bin/` and the test suite aligned.

---

## Repository layout and navigation

The top‑level `AGENTS.md` is the router for this project: it tells you which
directory‑specific `AGENTS.md` file to read before editing a given area. At a
glance:

- `probes/` — probe scripts and their authoring contract.
- `schema/`, `catalogs/` — JSON schemas and catalog instances.
- `src/` — Rust library and shared runtime logic.
- `src/bin/` — Rust helpers that back the `probe` CLI and helpers in `bin/`.
- `tests/` — integration suite that enforces contracts.
- `tools/` — authoring and contract‑gate tooling.
- `docs/` — human‑readable explanations for schemas, probes, and boundary
  objects.

Before you change behavior, skim:

- `AGENTS.md` at the repo root,
- the `AGENTS.md` for the directory you are touching, and
- any relevant docs in `docs/`.

Those files explain the contracts that code and tests are expected to uphold. The tests in `tests/` are intentionally opinionated and high‑coverage: keeping
them green is the easiest way to ensure usage remains compatible with the
contracts described above.