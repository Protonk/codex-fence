# AGENTS.md

If you are reading this, you have already seen `README.md` and `CONTRIBUTING.md` and you are ready to change something. This file is the shared contract for all agents—human or automated—working in this repository. It explains how to route yourself to the right place and what expectations apply everywhere, regardless of which subsystem you touch. Think of it as the index of agent responsibilities. 

## How to use this file

Treat this document as a router. Once you know which part of the tree you are touching, defer to the `AGENTS.md` in that directory and obey its contract. 

* `probes/AGENTS.md` — Probe Author contract: one observable action per script, cfbo-v1 emission rules, capability selection, and how to use helpers like `emit-record`, `portable-path`, and `json-extract`.
* `tests/AGENTS.md` — Test harness contract: how `cargo test` is wired, where fixtures live, and how guard rails map to specific contracts (schema, catalog, CLI, workspace). 
* `src/AGENTS.md` — Structure and expectations for Rust code under `src/`, excluding the helper CLIs.
* `src/bin/AGENTS.md` — Guarantees for the Rust helper binaries (`codex-fence`, `fence-run`, `emit-record`, `detect-stack`, etc.); how their CLIs and stack metadata must remain stable over time. 
* `tools/AGENTS.md` — Contracts for helper scripts under `tools/` and how they fit into supported workflows.
* `docs/AGENTS.md` — How explanatory docs relate to machine contracts; when you add or change an explainer in `docs/`, this is the place that says how it should track the schemas and tests. 

If you are unsure where a change belongs, pick the directory that contains the files you are editing and start with its `AGENTS.md`.

## Shared expectations for all agents

Several expectations apply no matter which subsystem you touch. These are the habits that let aggressive automation and human contributors coexist safely.

* Use the supported workflows. For probes, iterate with `tools/validate_contract_gate.sh --probe <id>` or `make probe PROBE=<id>` to get a fast, local contract gate before running the full suite. For repository-wide checks, use `cargo test` (or `cargo test --test suite`) as the single entry point for Rust guard rails, including schema validation and contract enforcement. 
* Treat `bin/codex-fence` as the top-level CLI for `--bang`, `--rattle`, `--listen`, and `--test`. It delegates to Rust helpers; keep its behavior aligned with the Makefile defaults and existing harness scripts rather than re-implementing probe logic in new places. 
* Preserve the portability stance described in `README.md` and `CONTRIBUTING.md`: scripts must run under macOS `/bin/bash 3.2` and inside the `codex-universal` container, using the Rust helpers shipped in `bin/`. When you change helper CLIs under `src/bin/`, rebuild them into `bin/` via `make build-bin` so Probes, tests, and tools see a consistent runtime. 
* Do not introduce new runtime dependencies beyond Bash and the existing Rust binaries. If you need new behavior, either express it in Bash or extend the Rust helpers and rebuild; do not add another interpreter or service to the runtime data path. 
* Canonicalize paths before you enforce workspace or probe boundaries. Use `bin/portable-path realpath|relpath` instead of ad-hoc `realpath`, `readlink`, or string slicing; the helpers exist to keep path handling portable and auditable. 
* Keep new policy in machine artifacts—schemas, probes, tests, tools. Documentation and AGENTS files explain those artifacts; they do not replace them. If you change a contract, there should be a schema and/or test that encodes it, and the relevant `*/AGENTS.md` should point to that enforcement. 

If you follow these rules—use the standard loops, stay within the dependency and portability boundaries, and encode policy in code + schemas + tests—you can make large changes with confidence. The guard rails are designed to reject inconsistent work, not to block you from trying.

## Repository layout snapshot

For quick orientation, this is how the tree is organized. Use it to decide which `AGENTS.md` to read next and to understand where your change sits in the larger system. 

| Path      | Purpose / Notes                                                                                                                                         |   |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | - |
| `bin/`    | Prebuilt Rust helper binaries (`codex-fence`, `fence-run`, `emit-record`, `portable-path`, `detect-stack`, etc.) synced from the sources in `src/bin/`. |   |
| `docs/`   | Human-readable explanations for schemas, probes, and boundary objects; kept aligned with machine contracts like `schema/*.json` and the tests.          |   |
| `probes/` | Flat directory of `<probe_id>.sh` scripts plus `probes/AGENTS.md`, the only code that directly exercises the sandboxed runtime.                         |   |
| `schema/` | Machine-readable capability catalog (`capabilities.json`) and boundary-object schema JSON (`boundary_object.json`) consumed by tooling.                 |   |
| `src/`    | Rust sources for the CLI and helpers, including implementations for every binary under `bin/`.                                                          |   |
| `target/` | Cargo build artifacts created by `cargo build` or `cargo test`; safe to delete when you need a clean rebuild.                                           |   |
| `tests/`  | Rust guard rails (`tests/suite.rs`), shared helpers, and fixtures that enforce the contracts under `cargo test`.                                        |   |
| `tmp/`    | Scratch space for probe and test runs; populated with ephemeral `.tmp*` directories that are safe to purge.                                             |   |
| `tools/`  | Developer tooling (validation scripts, adapters, contract gates) used by the supported workflows described above.                                       |   |

When in doubt, stop here, pick the directory you are about to change, and read its `AGENTS.md` end-to-end before editing. That is the main “rule of engagement” in this repository.
