build:
	clang plonk_inject.c -o inject.dylib -shared -Ldeps/ -lfrida-gum
	cargo build

.PHONY: build
