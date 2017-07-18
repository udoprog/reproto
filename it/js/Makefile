BABEL ?= babel --presets es2015
OUT := $(CURDIR)/target

all:
	@$(BABEL) src generated --out-dir $(OUT)
	@echo -e "#!/usr/bin/env bash\nexec env NODE_PATH="$(OUT)" node $(CURDIR)/target/script.js" > script.sh
	@chmod +x script.sh
