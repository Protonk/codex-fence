# Capability Catalog Guide 

This document summarizes the structure of `capabilities.json` so that humans and agents can quickly see what kinds of capabilities are modeled and decide whether to inspect the full catalog.

> NOTE: This document exists to help agents understand the structure of the catalog, NOT to provide additional technical or policy information.

Read this document to see how each field in `capabilities.json` is structured and interpreted, and how capability entries connect to probes and supporting references.  

`capabilities.json` begins with the "`schema_version`" which is **3**.

## Catalog scope and shared references

- The `scope` block sets the boundary for the catalog:
  - `description` Short title description of the catalog.
  - `notes` explains how we expect the catalog to grow (new capabilities, gradual expansion).
  - `policy_layers` lists the modeled layers of the stack (e.g., `os_sandbox`, `agent_runtime`).
  - `categories` gives short descriptions and counts for each capability category.
  - `limitations` documents what is intentionally out of scope for this catalog.
- The `docs` map is the canonical bibliography. Each key (e.g., `apple_sandbox_guide`, `deep_dive_agent_sandboxes`) acts as a stable handle. Capabilities reference these keys in `sources[*].doc`.

---

## Capability entries

Capability entries are a record of what we know about how the stack can mediate an action. Each entry is structured data about a particular feature or capability of the security policy surface, identified uniquely by an `id` — a short, snake-cased slug such as `cap_fs_write_workspace_tree`.

---

### Policy and categories at a glance

Total capabilities: **22**

#### Policies
| Layer ID        | Description                                                                                  | Capability count |
|-----------------|----------------------------------------------------------------------------------------------|------------------|
| `os_sandbox`    | Seatbelt / macOS sandbox behavior as seen by child processes.                                | 18               |
| `agent_runtime` | Codex agent orchestration and approvals policy around sandboxed runs.                        | 4                |

#### Categories

| Category ID            | Primary layer   | Description                                                                                 | Capability count |
|------------------------|-----------------|---------------------------------------------------------------------------------------------|------------------|
| `filesystem`           | `os_sandbox`    | Access to workspace trees, system roots, user content, and symlink behavior.               | 6                |
| `process`              | `os_sandbox`    | Process creation, execution, and sandbox escalation behavior.                              | 3                |
| `network`              | `os_sandbox`    | Outbound connectivity, localhost-only patterns, and network disablement.                   | 3                |
| `sysctl`               | `os_sandbox`    | Access to OS configuration and hardware information via `sysctl`.                          | 2                |
| `ipc`                  | `os_sandbox`    | Inter-process communication via Mach and related primitives.                               | 1                |
| `sandbox_profile`      | `os_sandbox`    | Profile structure, logging, and parameterization features.                                 | 3                |
| `agent_sandbox_policy` | `agent_runtime` | Session-level approvals, trust lists, sandbox env markers, and default sandboxing behavior.| 4                |

Capability counts in this guide are descriptive; `capabilities.json` is the source of truth.

---

### Example

```yaml
- id: cap_fs_read_workspace_tree
  category: filesystem
  layer: os_sandbox
  status: core
  description: Ability for commands to read files anywhere under the Codex workspace root(s).
  operations:
    allow:
      - file-read*
      - file-read-data
      - file-read-metadata
    deny: []
  meta_ops: []
  agent_controls: []
  level: medium
  notes: >
    This is the “normal” mode for Auto / Full access in Codex – agents must be able to read the
    checked-out project tree, but not arbitrary user directories. Expect to be implemented as
    allow file-read* (subpath WRITABLE_ROOT_i) for each workspace root, with deny default.
  sources:
    - doc: apple_sandbox_guide
      section: "2, 5.2 – File operations and allow/deny model"
    - doc: chromium_sandbox_v2
      section: "SBPL example using (subpath (param \"USER_HOME_DIR\"))"
    - doc: run_code_sandbox
      section: "Profile that allows read/write to $PWD only"
    - doc: deep_dive_agent_sandboxes
      section: "Codex Auto mode workspace access and SandboxPolicy.writable_roots"
```

---

### High-level categorization

Capabilities are categorized by where they are controlled (the `layer`) and what sorts of actions they mediate (`category`). 

- `layer` — tag noting **where** the rule lives:
  - `os_sandbox` — Seatbelt / macOS kernel sandbox policy as seen by child processes.
  - `agent_runtime` — Codex agent orchestration and policy around sandboxed runs, not OS-level Seatbelt rules.
- `category` — bucket (`filesystem`, `process`, `network`, `sysctl`, `ipc`, `sandbox_profile`, `agent_sandbox_policy`) for what the rule does. Capabilities listed in the catalog mediate mostly non-overlapping interactions; a call to the file system is distinct from reading `kern.boottime`. `category` should be the primary domain of the mediation.
  - `filesystem` — workspace roots, `.git` isolation, user/system directories, symlink handling, other file I/O rules.
  - `process` — exec/fork semantics, helper tools, and child-process policy.
  - `network` — outbound connectivity, loopback allowances, or explicit denials.
  - `sysctl` — kernel parameter reads such as `sysctl -n hw.ncpu`.
  - `ipc` — Mach services and other inter-process messaging.
- `sandbox_profile` — Seatbelt profile structure, logging, and parameterization. These capabilities often describe how the profile is written (e.g., parameters, default-deny posture) rather than specific allow/deny operations.
  - `agent_sandbox_policy` — how the agent uses or exposes the sandbox (approvals, trust lists, default sandboxing, env markers).

---

### Behavioral detail

- `description` — concise, user-facing summary of what the capability defends or permits.
- `operations` — `{allow: [...], deny: [...]}` lists of SBPL primitives required for the capability. These are raw Seatbelt operations (e.g., `file-read*`, `mach-lookup`), not policy keywords, and should only include the primitives that matter for the described behavior. For some structural capabilities (especially in the sandbox_profile category), `allow` and `deny` may both be empty, indicating that the capability is about how the profile is written (parameters, default-deny posture, logging), not about an additional syscall being allowed or denied.
- `meta_ops` — `sandbox-meta:*` tags that describe profile techniques (default deny, parameterization, debug/trace logging). When this list is empty, we are not modeling any additional non-SBPL/profile mechanisms for that capability.
- `agent_controls` — `agent-policy:*` tags describing agent-level knobs such as trust lists, approval modes, or sandbox env markers. When this list is empty, the capability is not mediated by any explicit agent-level control beyond the core policy.

---

### Guidance and provenance

Catalog entries are not exhausted by the structured fields alone. Useful information is held in free text in `notes`, and `sources` contain pointers to where we learned about the behavior.

- `notes` — probe-author hints: how to trigger the behavior, known tricky paths, or anything we learned by testing it.
- `sources` — list of `{doc, section, url_hint?}` objects pointing back to entries in the `docs` map. Include at least one reference for every capability so downstream consumers know where our understanding of the feature came from.
