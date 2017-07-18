SCRIPT_INPUT=$(CURDIR)/../tools/script-input

ROOT ?= ../..

DEFAULT_TOOL := $(ROOT)/target/debug/reproto

DIFF ?= diff
RSYNC ?= rsync
TOOL ?= $(DEFAULT_TOOL)
CARGO ?= cargo

EXPECTED = expected
OUTPUT = output
PATHS := proto $(PATHS)

TARGETS ?= test
FILTERED ?=

java_out = $(OUTPUT)/java
python_out = $(OUTPUT)/python
js_out = $(OUTPUT)/js
rust_out = $(OUTPUT)/rust
doc_out = $(OUTPUT)/doc

java_expected = $(EXPECTED)/java
python_expected = $(EXPECTED)/python
js_expected = $(EXPECTED)/js
rust_expected = $(EXPECTED)/rust
doc_expected = $(EXPECTED)/doc

PYTHON_EXTRA ?=
JAVA_EXTRA ?= -m builder
JS_EXTRA ?=
RUST_EXTRA ?=
DOC_EXTRA ?=

SUITES ?= python java js rust doc
PROJECTS ?= python java js rust

# projects that are filtered
EXCLUDE ?=
# projects that are supported after checking that necessary tools are available
INCLUDE ?= %

SUITES := $(filter $(INCLUDE), $(filter-out $(EXCLUDE), $(SUITES)))
PROJECTS := $(filter $(INCLUDE), $(filter-out $(EXCLUDE), $(PROJECTS)))

PACKAGES := $(TARGETS:%=--package %)

suite_targets := $(SUITES:%=suite-%)
suite_diffs := $(SUITES:%=suitediff-%)
suite_updates := $(SUITES:%=suiteupdate-%)

project_targets := $(PROJECTS:%=project-%)
project_diffs := $(PROJECTS:%=projectdiff-%)
project_updates := $(PROJECTS:%=projectupdate-%)

paths := $(PATHS:%=--path %)

DEBUG ?= no

ifeq ($(DEBUG),yes)
O :=
reproto := $(TOOL) --debug
else
O := @
reproto := $(TOOL)
endif

java_suite := java $(JAVA_EXTRA)
java_project := java -m fasterxml -o workdir-java/target/generated-sources/reproto

js_suite := js $(JS_EXTRA)
js_project := js -o workdir-js/generated

python_suite := python $(PYTHON_EXTRA)
python_project := python -o workdir-python/generated

rust_suite := rust $(RUST_EXTRA)
rust_project := rust -o workdir-rust/src --package-prefix generated

doc_suite := doc --skip-static $(DOC_EXTRA)

.PHONY: all clean suites projects update update-projects

all: suites projects

update: update-suites update-projects

clean-projects:
	$Orm -rf workdir-*
	$Orm -rf output-*

clean-suites:
	$Orm -rf output

clean: clean-projects clean-suites

suites: $(suite_targets) $(suite_diffs)

update-suites: $(suite_targets) $(suite_updates)

projects: $(project_targets) $(project_diffs)

update-projects: $(project_targets) $(project_updates)

suiteupdate-%:
	$Oecho "Updating Suite: $*"
	$O$(RSYNC) --delete -ra $($*_out)/ $($*_expected)/

suitediff-%:
	$Oecho "Verifying Diffs: $*"
	$O$(DIFF) -ur $($*_expected) $($*_out)

# rule to build suite output
suite-%: $(TOOL)
	$Oecho "Suite: $*"
	$O$(reproto) compile $($*_suite) -o $($*_out) $(paths) $(PACKAGES)

# rule to diff a projects expected output, with actual.
projectdiff-%:
	$Oecho "Diffing Project: $*"
	$O$(DIFF) -ur expected-$* output-$*

# rule to update a projects expected output, with its actual
projectupdate-%:
	$Oecho "Updating Project: $*"
	$O$(RSYNC) --delete -ra output-$*/ expected-$*/

# rule to build output for a project
project-%: $(TOOL)
	$O$(RSYNC) --delete -ra ../$*/ workdir-$*
	$O$(reproto) compile $($*_project) $(paths) $(PACKAGES)
	$Ocd workdir-$* && make
	$O$(SCRIPT_INPUT) workdir-$*/script.sh

$(DEFAULT_TOOL):
	$Oecho "Building $(DEFAULT_TOOL)"
	$O$(CARGO) build
