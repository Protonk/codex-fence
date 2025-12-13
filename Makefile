# fencerunner Makefile -- single entry points for builds and tests.

SHELL := /bin/bash
CARGO ?= cargo

.PHONY: build test clean-bin

# Remove all generated helpers while preserving the repo root sentinel.
clean-bin:
	find bin -type f ! -name '.gitkeep' -delete

# Rebuild and resync every helper binary into bin/.
build: clean-bin
	$(CARGO) clean -p fencerunner
	tools/sync_bin_helpers.sh

# Always run tests against freshly rebuilt helpers.
test: build
	$(CARGO) test --test suite
