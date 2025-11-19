SHELL := /bin/bash

ALL_PROBE_SCRIPTS := $(sort $(wildcard probes/*.sh))
PROBES ?= $(patsubst probes/%.sh,%,$(ALL_PROBE_SCRIPTS))
PROBE_SCRIPTS := $(foreach probe,$(PROBES),$(wildcard probes/$(probe).sh))
MISSING_PROBES := $(filter-out $(patsubst probes/%.sh,%,$(PROBE_SCRIPTS)),$(PROBES))

ifneq ($(strip $(MISSING_PROBES)),)
$(error Missing probe scripts: $(MISSING_PROBES))
endif
OUTDIR := out
PROBE ?=

HAS_CODEX := $(shell command -v codex >/dev/null 2>&1 && echo yes || true)
ifeq ($(HAS_CODEX),yes)
DEFAULT_MODES := baseline codex-sandbox codex-full
else
DEFAULT_MODES := baseline
endif

MODES ?= $(DEFAULT_MODES)

MATRIX_TARGETS := $(foreach mode,$(MODES),$(addprefix $(OUTDIR)/,$(addsuffix .$(mode).json,$(PROBES))))

.PHONY: all matrix clean test validate-capabilities probe

all: matrix

matrix: $(OUTDIR) $(MATRIX_TARGETS)
	@printf "Wrote %s records to %s\n" "$(words $(MATRIX_TARGETS))" "$(OUTDIR)"

$(OUTDIR):
	mkdir -p $@

define PROBE_template
$(OUTDIR)/$(1).baseline.json: $(2) | $(OUTDIR)
	bin/fence-run baseline $(2) > $$@

$(OUTDIR)/$(1).codex-sandbox.json: $(2) | $(OUTDIR)
	bin/fence-run codex-sandbox $(2) > $$@

$(OUTDIR)/$(1).codex-full.json: $(2) | $(OUTDIR)
	bin/fence-run codex-full $(2) > $$@
endef

$(foreach script,$(PROBE_SCRIPTS), \
  $(eval $(call PROBE_template,$(notdir $(basename $(script))),$(script))) \
)

clean:
	rm -rf $(OUTDIR)

test:
	tests/run.sh

probe:
	@if [[ -z "$(PROBE)" ]]; then \
		echo "Usage: make probe PROBE=<probe_id_or_path>"; \
		exit 1; \
	fi
	tests/run.sh --probe "$(PROBE)"

validate-capabilities:
	tools/validate_capabilities.sh
