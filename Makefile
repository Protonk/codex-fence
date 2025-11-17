SHELL := /bin/bash
PROBE_SCRIPTS := $(wildcard probes/*.sh)
PROBES := $(patsubst probes/%.sh,%,$(PROBE_SCRIPTS))
OUTDIR := out

HAS_CODEX := $(shell command -v codex >/dev/null 2>&1 && echo yes || true)
ifeq ($(HAS_CODEX),yes)
DEFAULT_MODES := baseline codex-sandbox codex-full
else
DEFAULT_MODES := baseline
endif

MODES ?= $(DEFAULT_MODES)

MATRIX_TARGETS := $(foreach mode,$(MODES),$(addprefix $(OUTDIR)/,$(addsuffix .$(mode).json,$(PROBES))))

.PHONY: all matrix clean test validate-capabilities

all: matrix

matrix: $(OUTDIR) $(MATRIX_TARGETS)
	@printf "Wrote %s records to %s\n" "$(words $(MATRIX_TARGETS))" "$(OUTDIR)"

$(OUTDIR):
	mkdir -p $@

$(OUTDIR)/%.baseline.json: probes/%.sh | $(OUTDIR)
	bin/fence-run baseline $* > $@

$(OUTDIR)/%.codex-sandbox.json: probes/%.sh | $(OUTDIR)
	bin/fence-run codex-sandbox $* > $@

$(OUTDIR)/%.codex-full.json: probes/%.sh | $(OUTDIR)
	bin/fence-run codex-full $* > $@

clean:
	rm -rf $(OUTDIR)

test:
	tests/run.sh

validate-capabilities:
	tools/validate_capabilities.sh
