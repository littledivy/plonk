```shell
cargo build

target/debug/example_cli
Hello, world! x1

# Change something in library/
# Only build library
cargo rustc --crate-type=dylib -p example_lib --features=hot_swap

# Run binary with hot swap
VERBOSE=* \
SYMBOL=say_hello \
NEW_SYMBOL=say_hello \
PLONK_LIBRARY=/Users/divy/gh/deno_build/example_rust/target/debug/libexample_lib.dylib \
DYLD_INSERT_LIBRARIES="/Users/divy/gh/deno_build/inject.dylib" \
target/debug/example_cli

[*] Plonking say_hello in /Users/divy/gh/deno_build/example_rust/target/debug/libexample_lib.dylib
[*] Old address: 0x1029c0844
[*] New address: 0x103633db8
===
Hello, world! x2
```

