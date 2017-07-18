CARGO ?= cargo

BINARY := $(CURDIR)/target/debug/rust

all:
	$(CARGO) build
	@echo -e "#!/usr/bin/env bash\nexec $(BINARY)" > script.sh
	@chmod +x script.sh
