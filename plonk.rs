/*
 * Copyright (c) 2023 Divy Srivastava
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
 */

use cargo_metadata::{MetadataCommand, Node, Package, PackageId};
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const HELP: &'static str = "\
plonk

USAGE:
    plonk [COMMAND] [FLAGS]

FLAGS:
    -h, --help       Prints help information
    -v, --verbose    Verbose output
    -p, --package    Package to build
    -s, --symbol     Hot reload for a specific symbol
    -r, --release    Build in release mode
    -w, --watch      Watch for changes and rebuild

SUBCOMMANDS:
    build    Compile the package
    run      Run the binary
";

#[derive(Default)]
struct Options {
    // -v, --verbose
    verbose: bool,

    // -p, --package
    package: String,

    // -r, --release
    release: bool,

    // -s, --symbol
    symbol: Option<String>,

    // -w, --watch
    watch: bool,

    _internal_meta: bool,
}

fn main() {
    let mut pargs = pico_args::Arguments::from_env();

    let cmd = pargs.subcommand().unwrap();

    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        return;
    }

    let mut opts = Options {
        verbose: pargs.contains(["-v", "--verbose"]),
        package: pargs
            .value_from_str(["-p", "--package"])
            .unwrap_or_else(|_| ".".to_string()),
        release: pargs.contains(["-r", "--release"]),
        symbol: pargs.value_from_str(["-s", "--symbol"]).ok(),
        watch: pargs.contains(["-w", "--watch"]),
        ..Default::default()
    };

    match cmd.as_deref() {
        Some("build") => {
            build(&mut opts);
        }
        Some("run") => run(&mut opts),
        _ => {
            println!("No command specified");
            print!("{}", HELP);
        }
    }
}

fn watch<R>(pargs: &mut Options, fn_: fn(&mut Options) -> R) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer =
        new_debouncer(Duration::from_millis(100), tx).expect("Failed to create watcher");

    let local_deps = find_local_deps().expect("Failed to find local deps");

    for dep in local_deps {
        debouncer
            .watcher()
            .watch(&dep, RecursiveMode::Recursive)
            .expect("Failed to watch");
    }

    fn_(pargs);
    for _event in rx.iter() {
        fn_(pargs);
    }
}

fn build(pargs: &mut Options) -> Option<cargo_metadata::Artifact> {
    if pargs.watch {
        pargs.watch = false;
        watch(pargs, build);
    }

    let mut cargo = Command::new("cargo");
    cargo
        .arg("rustc")
        .arg("--crate-type=dylib")
        .arg("-p")
        .arg(&pargs.package);

    if pargs.release {
        cargo.arg("--release");
    }

    if pargs.verbose {
        cargo.arg("-vv");
    }

    if pargs._internal_meta {
        cargo
            .arg("--message-format=json-render-diagnostics")
            .stdout(std::process::Stdio::piped());
    }

    let mut cargo = cargo.spawn().expect("Failed to spawn cargo build");

    let cargo_status = cargo.wait().expect("Failed to wait for cargo build");
    assert!(cargo_status.success(), "Cargo build failed");

    if pargs._internal_meta {
        let reader = std::io::BufReader::new(cargo.stdout.take().unwrap());
        for message in cargo_metadata::Message::parse_stream(reader) {
            let message = message.expect("Failed to parse message");
            match message {
                cargo_metadata::Message::CompilerArtifact(artifact) => {
                    if artifact.target.kind.contains(&"dylib".to_string()) {
                        return Some(artifact);
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn get_bin_crates(meta: &cargo_metadata::Metadata, release: bool) -> Option<String> {
    for pkg in meta.packages.iter() {
        for bin in pkg.targets.iter() {
            if bin.kind.contains(&"bin".to_string()) {
                let mut path = meta.target_directory.clone();
                path.push(if release { "release" } else { "debug" });
                path.push(&bin.name);

                return Some(path.to_string());
            }
        }
    }
    None
}

// https://github.com/watchexec/cargo-watch/blob/da7e7f5c631adffce74be97949e7aadfaff1c953/src/options.rs#L165
fn find_local_deps() -> Result<Vec<PathBuf>, String> {
    let metadata = MetadataCommand::new()
        .exec()
        .map_err(|e| format!("Failed to execute `cargo metadata`: {}", e))?;

    let resolve = match metadata.resolve {
        None => return Ok(Vec::new()),
        Some(resolve) => resolve,
    };
    let id_to_node =
        HashMap::<PackageId, &Node>::from_iter(resolve.nodes.iter().map(|n| (n.id.clone(), n)));
    let id_to_package = HashMap::<PackageId, &Package>::from_iter(
        metadata.packages.iter().map(|p| (p.id.clone(), p)),
    );

    let mut pkgids_seen = HashSet::new();
    let mut pkgids_to_check = Vec::new();
    match resolve.root {
        Some(root) => pkgids_to_check.push(root),
        None => pkgids_to_check.extend_from_slice(&metadata.workspace_members),
    };

    // The set of directories of all packages we are interested in.
    let mut local_deps = HashSet::new();

    while !pkgids_to_check.is_empty() {
        let current_pkgid = pkgids_to_check.pop().unwrap();
        if !pkgids_seen.insert(current_pkgid.clone()) {
            continue;
        }

        let pkg = match id_to_package.get(&current_pkgid) {
            None => continue,
            Some(&pkg) => pkg,
        };

        // This means this is a remote package. Skip!
        if pkg.source.is_some() {
            continue;
        }

        // This is a path to Cargo.toml.
        let mut path = pkg.manifest_path.clone();
        // We want the directory it's in.
        path.pop();
        local_deps.insert(path.into_std_path_buf());

        // And find dependencies.
        if let Some(node) = id_to_node.get(&current_pkgid) {
            for dep in &node.deps {
                pkgids_to_check.push(dep.pkg.clone());
            }
        }
    }

    Ok(local_deps.into_iter().collect::<Vec<PathBuf>>())
}

fn run(pargs: &mut Options) {
    if pargs.watch {
        pargs.watch = false;
        watch(pargs, run);
    }

    pargs._internal_meta = true;
    let artifact = build(pargs).expect("Failed to build");

    let cmd = cargo_metadata::MetadataCommand::new();
    let meta = cmd.exec().expect("Failed to get metadata");

    let bin = get_bin_crates(&meta, pargs.release).expect("Failed to get binary crate");

    let library_path = artifact.filenames[0].clone();
    let mut lib = Command::new(&bin);
    if pargs.verbose {
        lib.env("VERBOSE", "y");
    }

    if let Some(symbol) = &pargs.symbol {
        lib.env("SYMBOL", symbol);
    } else {
        println!("No symbol specified. Use -s to specify a function");
        print!("{}", HELP);
        return;
    }

    lib.env("PLONK_LIBRARY", library_path)
        .env("DYLD_INSERT_LIBRARIES", "../inject.dylib");

    let mut lib = match lib.spawn() {
        Ok(lib) => lib,
        Err(_) => {
            println!("Failed to spawn binary: {}", bin);
            println!("Did you forget to build the binary with `cargo build`?");

            println!("{}", HELP);
            return;
        }
    };

    lib.wait().expect("Failed to wait for bin");
}