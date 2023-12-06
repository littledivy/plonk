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
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

mod plonk_inject;

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
    -b, --bin        Manually specify binary package

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

    // -b, --bin
    bin: Option<String>,

    // -r, --release
    release: bool,

    // -s, --symbol
    symbol: Option<String>,

    // -w, --watch
    watch: bool,

    _internal_meta: bool,
    forward: Vec<OsString>,

    #[allow(dead_code)]
    watch_cache: WatchCache,
}

#[derive(Default)]
struct WatchCache {
    #[allow(dead_code)]
    bin_symbol: Option<String>,
}

const INJECT_DYLIB: &'static str = env!("PLONK_INJECT_DYLIB");

fn main() {
    // `from_vec` takes `OsString`, not `String`.
    let mut args: Vec<_> = std::env::args_os().collect();
    args.remove(0); // remove the executable path.

    // Find and process `--`.
    let forward = if let Some(dash_dash) = args.iter().position(|arg| arg == "--") {
        // Store all arguments following ...
        let later_args = args.drain(dash_dash + 1..).collect();
        // .. then remove the `--`
        args.pop();
        later_args
    } else {
        Vec::new()
    };

    let mut pargs = pico_args::Arguments::from_vec(args);

    let mut cmd = pargs.subcommand().unwrap();

    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        return;
    }

    let mut opts = Options {
        verbose: pargs.contains(["-v", "--verbose"]),
        package: pargs
            .value_from_str(["-p", "--package"])
            .unwrap_or_else(|_| ".".to_string()),
        bin: pargs.value_from_str(["-b", "--bin"]).ok(),
        release: pargs.contains(["-r", "--release"]),
        symbol: pargs.value_from_str(["-s", "--symbol"]).ok(),
        watch: pargs.contains(["-w", "--watch"]),
        forward,
        ..Default::default()
    };

    // Invoked as `cargo plonk`
    if matches!(cmd.as_deref(), Some("plonk")) {
        cmd = pargs.subcommand().unwrap();
    }

    let remaining = pargs.finish();
    if !remaining.is_empty() {
        println!("Unknown arguments: {:?}", remaining);
        print!("{}", HELP);
        return;
    }

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
        .env("RUSTFLAGS", "-C prefer-dynamic")
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
        cargo.arg("--message-format=json-render-diagnostics");
    }

    cargo.stderr(std::process::Stdio::inherit());

    let cargo = cargo.output().expect("Failed to spawn cargo build");
    assert!(cargo.status.success());

    if pargs._internal_meta {
        let cursor = std::io::Cursor::new(&cargo.stdout[..]);
        let reader = std::io::BufReader::new(cursor);
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

fn get_bin_crates(meta: &cargo_metadata::Metadata, release: bool) -> Vec<(String, String)> {
    let mut bins = Vec::new();
    for pkg in meta.packages.iter() {
        for bin in pkg.targets.iter() {
            if bin.kind.contains(&"bin".to_string()) {
                let mut path = meta.target_directory.clone();
                path.push(if release { "release" } else { "debug" });
                path.push(&bin.name);

                bins.push((pkg.name.clone(), path.to_string()));
            }
        }
    }
    bins
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

fn rustc_sysroot() -> PathBuf {
    let mut cmd = Command::new("rustc");
    cmd.arg("--print").arg("sysroot");
    let cmd = cmd.output().expect("Failed to spawn rustc");
    let stdout = std::str::from_utf8(&cmd.stdout[..]).expect("Failed to parse rustc output");
    PathBuf::from(stdout.trim())
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

    let bins = get_bin_crates(&meta, pargs.release);
    let (_, bin) = match &pargs.bin {
        Some(package) => match bins.iter().find(|(pkg, _)| pkg == package) {
            None => {
                println!("No binary found with name: {}", package);
                println!("Available binaries: {:?}", bins);
                return;
            }
            Some(b) => b,
        },
        None => {
            if bins.len() > 1 {
                println!("Multiple binaries found. Use -b to specify a binary");
                println!("Available binaries: {:?}", bins);
                return;
            } else if bins.len() == 0 {
                println!("No binaries found");
                return;
            }
            bins.first().unwrap()
        }
    };

    let library_path = artifact.filenames[0].clone();
    let mut lib = Command::new(&bin);
    if pargs.verbose {
        lib.env("VERBOSE", "y");
    }

    #[cfg(target_os = "unix")]
    {
        if let Some(symbol) = &pargs.symbol {
            let old_symbol = pargs
                .watch_cache
                .bin_symbol
                .clone()
                .or_else(|| find_symbol(&bin, &pargs.package, symbol));
            match old_symbol {
                Some(old_symbol) => {
                    lib.env("SYMBOL", &old_symbol);
                    pargs.watch_cache.bin_symbol = Some(old_symbol);
                }
                None => {
                    println!("Failed to find function symbol `{}` in {}", symbol, bin);
                    println!("See FAQ"); // TODO
                    return;
                }
            }

            let new_symbol = find_symbol(library_path.as_ref(), &pargs.package, symbol);
            match new_symbol {
                Some(new_symbol) => {
                    lib.env("NEW_SYMBOL", &new_symbol);
                }
                None => {
                    println!(
                        "Failed to find function symbol `{}` in {}",
                        symbol, library_path
                    );
                    println!("See FAQ"); // TODO
                    return;
                }
            };
        } else {
            println!("No symbol specified. Use -s to specify a function");
            print!("{}", HELP);
            return;
        }
    }

    lib.env("PLONK_LIBRARY", &library_path)
        .env("PLONK_BINARY", bin);
    #[cfg(target_os = "macos")]
    {
        lib.env("DYLD_INSERT_LIBRARIES", INJECT_DYLIB)
            .env("DYLD_LIBRARY_PATH", rustc_sysroot().join("lib"));
    }
    #[cfg(target_os = "linux")]
    {
        lib.env("LD_PRELOAD", INJECT_DYLIB)
            .env("LD_LIBRARY_PATH", rustc_sysroot().join("lib"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(sym) = &pargs.symbol {
            lib.env("SYMBOL", sym);
            lib.env("NEW_SYMBOL", sym);
        }
        lib.env(
            "PATH",
            rustc_sysroot().join("lib/rustlib/x86_64-pc-windows-msvc/lib"),
        );
    }

    for arg in &pargs.forward {
        lib.arg(arg);
    }

    if pargs.verbose {
        println!("[*] Running: {:?}", lib);
    }

    if cfg!(target_os = "windows") {
        let escaped = INJECT_DYLIB.replace("\\", "\\\\");
        unsafe { plonk_inject::inject(&mut lib, &escaped) };

        return;
    }

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

#[cfg(target_os = "unix")]
fn find_symbol(path: &str, package: &str, symbol: &str) -> Option<String> {
    let full_symbol = format!("{}::{}", package, symbol);
    let mut cmd = Command::new("nm");
    cmd.arg(path);
    let cmd = cmd.output().expect("Failed to spawn nm");

    let stdout = std::str::from_utf8(&cmd.stdout[..]).expect("Failed to parse nm output");
    let stdout = stdout.split("\n").collect::<Vec<&str>>();

    for line in stdout {
        let line = line.trim();
        let cols = line.split(" ").collect::<Vec<&str>>();
        if cols.len() < 3 {
            continue;
        }
        if cols[1] == "t" || cols[1] == "T" {
            if cols[2] == symbol {
                return Some(symbol.into());
            }

            #[cfg(target_os = "macos")]
            // _<symbol>.
            if cols[2] == format!("_{}", symbol) {
                return Some(symbol.to_string());
            }

            let demangled = rustc_demangle::demangle(cols[2]).to_string();
            if demangled.contains(&full_symbol) {
                #[cfg(target_os = "macos")]
                // Remove _ from _<symbol>.
                return Some(cols[2][1..].to_string());

                #[cfg(not(target_os = "macos"))]
                return Some(cols[2].to_string());
            }
        }
    }

    None
}
