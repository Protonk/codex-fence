# Contributing to codex-fence

Thanks for improving codex-fence! This project tries to stay lightweight and friendly to both human and AI contributors. The basics: keep probes single-purpose, lean on the helper library when you need shared utilities, and run the fast test suite before you send patches.

## Workflow overview

1. **Read the Probe Author contract (AGENTS.md)** – everything here assumes you already follow that contract.
2. **Use the helper library** – portable path helpers live in `tools/lib/helpers.sh`. Source it in a probe or tool if you need `portable_realpath` / `portable_relpath` instead of rolling your own interpreter detection.
3. **When touching Ruby tooling** – only parse YAML via `CodexFence::RubyYamlLoader` (`tools/lib/ruby_yaml_loader.rb`). It keeps the adapter compatible with both macOS system Ruby 2.6 and the container’s Ruby 3.x.
4. **Run the quick tests** – `make test` runs all Bash suites, including `baseline_no_codex_smoke`, which shadows `codex` in `PATH` to ensure the harness works on macOS without Codex installed. You should see: `static_probe_contract`, `capability_map_sync`, `boundary_object_schema`, `harness_smoke`, and `baseline_no_codex_smoke`.

## Tests and tooling

- `make test` – authoring sanity checks (must pass before submitting patches).
- `make validate-capabilities` – verifies every probe/fixture references real capability IDs.
- `make matrix MODES=baseline` – optional local run to emit all baseline boundary objects. Be careful running Codex modes unless you have the CLI installed.

### Portable helper usage

Example pattern for a probe:

```bash
repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." >/dev/null 2>&1 && pwd)
helpers_lib="${repo_root}/tools/lib/helpers.sh"
# shellcheck source=tools/lib/helpers.sh
source "${helpers_lib}"

canonical_path=$(portable_realpath "${attempt_path}")
```

Keep helpers pure—no global state, no side effects—so probes remain single-purpose. If you need a new helper, add it to `tools/lib/helpers.sh` instead of duplicating logic.

### Ruby adapter helper

Tools that need to parse `spec/capabilities.yaml` should load `CodexFence::RubyYamlLoader` instead of calling `YAML.safe_load` directly. Example:

```bash
ruby -I"${repo_root}/tools/lib" -rruby_yaml_loader … <<'RUBY'
require 'ruby_yaml_loader'
data = CodexFence::RubyYamlLoader.safe_load_file(path)
RUBY
```

This keeps the CLI-friendly Bash wrapper simple while ensuring cross-version compatibility.
