# codex-fence

`codex-fence` is a lightweight harness for poking at Codex security fences. It runs tiny "probe" scripts
under several run modes (baseline shell, Codex sandbox, and an experimental Codex full-access mode) and
records the results as structured JSON boundary objects. The tool never talks to models—it simply observes
how the runtime fence reacts to filesystem, network, process, or other system interactions.

## Requirements

- POSIX shell utilities + `bash`
- `jq`
- `make`
- The `codex` CLI (only if you plan to exercise Codex modes)

## Usage

Run a single probe in a chosen mode:

```sh
bin/fence-run baseline fs_outside_workspace
```

Matrix all probes across all modes and store the JSON output in `out/`:

```sh
make matrix
```

Each record uses the schema in `schema/boundary-object-v0.json`. Human-readable documentation lives in
`docs/boundary-object.md`, and instructions for adding probes live in `docs/AGENTS.md`.

## Why?

The "right" way to run an untrusted AI assistant is 
inside a container where it can't accidentally read 
your tax returns or delete your home directory. 
Nevertheless, I would agree with [Pierce Freeman](https://pierce.dev/notes/a-deep-dive-on-agent-sandboxes) 
and "wadger a large sum that almost no one does that."

Most developers working with `codex` CLI will do
so on a Mac where the sandboxing policy is officially deprecated
and mostly documented by curious outsiders. What
kinds of things can or can't Codex do in your stack?
Do you know? How would you know if things changed?

You'd know if you used `codex-fence`.