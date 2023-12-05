# plonk

Plonk is a development-time build tool for Rust projects.

```shell
cargo install cargo-plonk
```

![plonk_demo](https://github.com/littledivy/plonk/assets/34997667/bdc1e3c5-6740-42e7-b7b5-32c22cd45311)

```shell
# fn main() {
#  lib::say_hello();
# }
$ cargo build -p example_cli

# pub fn say_hello() {
#  println!("Hello x1");
# }
$ cargo plonk \
  run \
  --package example_lib \
  --symbol say_hello

Hello x1

$ echo "pub fn say_hello() {\n  println!('Hello x2');\n}" > example_lib/lib.rs

$ cargo plonk \
  run \
  --package example_lib \
  --symbol say_hello

Hello x2
```
