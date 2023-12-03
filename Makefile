build:
	clang inject.c -o inject.dylib -shared -Ldeps/ -lfrida-gum

.PHONY: build
