# Capabilities schema guide

`spec/capabilities.yaml` defines the subset of sandbox and agent behaviors that codex-fence models. This document stays next to the spec so authors can see how each field is intended to be used, how it maps to probes, and what supporting material belongs with every capability. It is not a Seatbelt encyclopedia—just the schema explainer for the catalog codex-fence currently ships with.

The current schema version is **2**, centered on macOS Seatbelt plus Codex agent policy. The structure anticipates future platforms and capability classes, so every field description below should be interpreted with that growth path in mind.

## Catalog scope and shared references

- The `scope` block sets the boundary for the catalog:
  - `description` summarizes what this slice of Seatbelt/agent behavior covers.
  - `platforms` lists the platforms currently modeled (today `[macos]`, tomorrow more).
  - `notes` explains how we expect the catalog to grow (new profiles, gradual expansion).
- The `docs` map is the canonical bibliography. Each key (e.g., `apple_sandbox_guide`, `deep_dive_agent_sandboxes`) acts as a stable handle. Capabilities reference these keys in `sources[*].doc`, keeping URLs centralized and updates surgical.

## Capability entry schema

Each object in `capabilities` carries four clusters of fields:

### Identity and grouping

- `id` — stable identifier the probes, prompts, and boundary objects emit. Never rename without migrating every consumer.
- `category` — high-level bucket (`filesystem`, `process`, `network`, `sysctl`, `ipc`, `sandbox_meta`, `agent_policy`). Choose the primary behavior being exercised, not every side effect.
- `platform` — list of operating systems the capability statement applies to. Default to `[macos]` today, expand as soon as probes exist for another platform.

### Enforcement context and lifecycle

- `layer` — clarifies **where** the rule lives:
  - `os_sandbox` for Seatbelt/kernel policy.
  - `sandbox_meta` for profile-construction mechanics (default deny, parameterization, logging).
  - `agent_policy` for Codex orchestration (approval prompts, sandbox toggles).
- `status` — `planned`, `experimental`, or `core`. Start every new entry at `experimental` until we have a reliable probe.
- `level` — fast severity/impact signal (`low`, `medium`, `high`).

### Behavioral detail

- `description` — concise, user-facing summary of what the capability defends or permits.
- `operations` — `{allow: [...], deny: [...]}` lists of SBPL primitives required for the capability. These are raw Seatbelt operations (e.g., `file-read*`, `mach-lookup`), not policy keywords, and should only include the primitives that matter for the described behavior.
- `meta_ops` — `sandbox-meta:*` tags that describe the profile techniques in play (default deny, argument templating, debug injectors, etc.).
- `agent_controls` — `agent-policy:*` tags describing agent-level knobs such as trust lists or approval requirements.

### Guidance and provenance

- `notes` — probe-author hints: how to trigger the behavior, known tricky paths, or anything we learned by testing it.
- `sources` — list of `{doc, section, url_hint?}` objects pointing back to entries in the `docs` map. Include at least one reference for every capability so downstream consumers know where the behavior came from.

## Categories and layers cheat sheet

- `filesystem` — workspace roots, `.git` isolation, user/system directories, symlink handling, other file I/O rules.
- `process` — exec/fork semantics, helper tools, and child-process policy.
- `network` — outbound connectivity, loopback allowances, or explicit denials.
- `sysctl` — kernel parameter reads such as `sysctl -n hw.ncpu`.
- `ipc` — Mach services and other inter-process messaging.
- `sandbox_meta` — mechanics of the sandbox profile itself.
- `agent_policy` — Codex-level coordination outside the kernel.

`layer` mirrors the above lenses:

- `os_sandbox` — enforced by the macOS Seatbelt runtime.
- `sandbox_meta` — “profile about profiles” behaviors (default deny posture, logging toggles).
- `agent_policy` — approvals flow, sandbox orchestration, and similar agent decisions.

## Working with spec/capabilities.yaml

- Probes **must** cite `id` values that already exist in the spec. If you need a new capability, add it to `spec/capabilities.yaml` in the same change that introduces the probe.
- “One probe per behavior” still allows multiple capability IDs when necessary; just ensure the payload makes that clear.
- Schema edits (new fields, enum values, or remapped layers) require synchronized updates to:
  1. `spec/capabilities.yaml`.
  2. This guide (`spec/AGENTS.md`) so future authors understand the field.
  3. Any logic that parses or emits capability metadata (probes, emit-record helpers, docs).
- When writing higher-level docs or prompts, link to capability IDs instead of describing rules ad hoc; that keeps the catalog authoritative.
