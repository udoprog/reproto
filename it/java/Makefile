MVN ?= mvn -q

ifeq ($(DEBUG),yes)
MVN := $(MVN) -X
else
MVN := $(MVN) -q
endif

all:
	@$(MVN) package
	@$(MVN) dependency:build-classpath -D mdep.outputFile=.classpath
	@echo -e "#!/usr/bin/env bash\nexec java -cp $$(cat .classpath):$(CURDIR)/target/classes Test" > script.sh
	@chmod +x script.sh
