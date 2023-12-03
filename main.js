import { $ } from "https://deno.land/x/dax/mod.ts";
import { walkSync } from "https://deno.land/std@0.208.0/fs/walk.ts";
import { parse } from "https://deno.land/std@0.208.0/flags/mod.ts";

const { d: dep } = parse(Deno.args, {

});

await $`cargo +nightly rustc --crate-type=dylib`.cwd(d);

const lib = `target/debug/lib${dep}.dylib`;

// cargo build -p deno
//
// init -s init_ops_and_esm -d deno_websocket -b deno
//
// build 
// build --watch
// run
// run --watch

