Plonk is a development-time build tool for Rust projects.

My main tool for building Deno.

1. Main binary crate `A` that imports `B`
2. Build `A` normally.
3. Build `B` as dylib and export new symbol
4. Inject B dylib and injector dylib that swaps out old symbol with new symbol.


## Building deno extensions with this

```shell
cargo build -p deno

cargo rustc --crate-type-dylib -p deno_websocket

export SYMBOL=init_ops_and_esm
export DYLD_INSERT_LIBRARIES="libdeno_websocket.dylib:injector.dylib"

target/debug/debug
```

But Rust ABI isn't stable? :nerd_face:

-> No one cares and is stupid enough to compile with 2 different version of rustc during development.
