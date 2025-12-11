# paging-stress helper

`paging-stress` is a tiny helper binary that allocates anonymous memory and
touches each page to apply light paging pressure. Probes invoke it with explicit
arguments and interpret only the exit code so the probe contract (single JSON
record, no stdout) stays intact.

## CLI

- `--megabytes <N>` — total allocation size in MiB (default: 8).
- `--passes <N>` — number of full sweeps to perform (default: 1).
- `--pattern <sequential|random>` — page access order (default: sequential).
- `--max-seconds <N>` — optional self-enforced timeout.
- `--help` — print usage.

Exit codes: `0` success, `1` invalid arguments, `2` internal error (allocation
or runtime failure), `3` timeout. The helper never writes to stdout; diagnostics
land on stderr only when arguments are invalid or an error/timeout occurs.

## How probes should use it

Probes remain thin Bash wrappers:

1. Choose small, test-friendly arguments.
2. Optionally wrap invocation in a timeout to classify hung runs.
3. Map exit codes into `observed_result` and payload fields before calling
   `bin/emit-record`.

No data is emitted on stdout—any structured payload must be collected by the
probe itself if/when we add such modes.

## Building

`make build` (or `tools/sync_bin_helpers.sh`) builds the helper via Cargo
and syncs it into `bin/paging-stress`. You can also run it directly with
`cargo run --bin paging-stress -- --megabytes 4 --passes 1`.
