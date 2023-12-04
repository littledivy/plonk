Plonk is a development-time build tool for Rust projects.

```shell
# fn main() {
#  lib::say_hello();
# }
$ cargo build -p example_cli

# #[no_mangle]
# pub fn say_hello() {
#  println!("Hello x1");
# }
$ cargo plonk \
  run \
  --package example_lib \
  --symbol say_hello

Hello x1

$ echo "#[no_mangle]\npub fn say_hello() {\n  println!('Hello x2');\n}" > example_lib/lib.rs

$ cargo plonk \
  run \
  --package example_lib \
  --symbol say_hello

Hello x2
```
