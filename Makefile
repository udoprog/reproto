.PHONY: all suites projects clean

DEFAULT_TOOL := $(CURDIR)/target/debug/reproto

PYTHON ?= python3
PROJECTS ?= $(shell PYTHON=$(PYTHON) tools/check-project-deps)
TOOL ?= $(DEFAULT_TOOL)
ENVIRONMENT := INCLUDE="$(PROJECTS)" PYTHON="$(PYTHON)" TOOL="$(TOOL)"
EACH := tools/for-each-it

CARGO_TARGET_DIR=$(CURDIR)/target

all: suites projects

update: update-suites update-projects

tests:
	cargo test --all

clean:
	cargo clean
	$(EACH) clean

# simplified set of suites
suites: $(TOOL)
	$(EACH) TOOL="$(TOOL)" suites

update-suites: $(TOOL)
	$(EACH) TOOL="$(TOOL)" update-suites

clean-suites: $(TOOL)
	$(EACH) TOOL="$(TOOL)" clean-suites

# extensive project-building test suites
projects: $(TOOL)
	$(EACH) $(ENVIRONMENT) --no-print-directory projects

update-projects: $(TOOL)
	$(EACH) $(ENVIRONMENT) --no-print-directory update-projects

clean-projects: $(TOOL)
	$(EACH) $(ENVIRONMENT) --no-print-directory clean-projects

help:
	@echo ""
	@echo "Please read 'Suites & Projects' in README.md"
	@echo ""
	@echo "Variables (specified like 'make VARIABLE=VALUE <target>'):"
	@echo "  PROJECTS=foo - only build the listed kinds of projects"
	@echo "  DEBUG=yes    - (very) verbose output"
	@echo "  EXCLUDE=rust - exclude the named targets"
	@echo "  INCLUDE=rust - only include the named targets"
	@echo ""
	@echo "Targets:"
	@echo "  all    - default target (suites projects)"
	@echo "  tests  - run unit tests for project"
	@echo "  clean  - clean build and tests"
	@echo "  update - update everything (update-suites update-projects)"
	@echo ""
	@echo "Suite Targets:"
	@echo "  suites        - run it suites"
	@echo "  update-suites - update expected output for it suites"
	@echo "  clean-suites  - clean it suites"
	@echo ""
	@echo "Project Targets:"
	@echo "  projects        - run it projects"
	@echo "  update-projects - update expected output for it projects"
	@echo "  clean-projects  - clean it projects"
	@echo ""
	@echo "Examples:"
	@echo "  Run all suites (very fast):"
	@echo "    make clean-suites suites"
	@echo "  A single set of suites:"
	@echo "    make -C it/test-match clean-suites suites"
	@echo "  A single set of projects:"
	@echo "    make -C it/test-match clean-projects projects"
	@echo ""

$(DEFAULT_TOOL):
	cargo build
