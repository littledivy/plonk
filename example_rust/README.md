```shell
cargo build

# Change something in library/
# Only build library
cargo rustc --crate-type=dylib -p example_lib --features=hot_swap

# Run binary with hot swap
SYMBOL=say_hello NEW_SYMBOL=say_hello_new \
    DYLD_INSERT_LIBRARIES="/Users/divy/gh/deno_build/example_rust/target/debug/libexample_lib.dylib:/Users/divy/gh/deno_build/inject.dylib" \
    target/debug/example_cli
```

