use std::process::Command;

const HELP: &'static str = "\
plonk

USAGE:
    plonk

FLAGS:
    -h, --help       Prints help information

SUBCOMMANDS:
    build    Build
    run      Run
";

fn main() {
    let mut pargs = pico_args::Arguments::from_env();

    let cmd = pargs.subcommand().unwrap();

    if pargs.contains(["-h", "--help"]) {
        print!("{}", HELP);
        return;
    }

    match cmd.as_deref() {
        Some("build") => build(&mut pargs),
        Some("run") => run(&mut pargs),
        _ => {
            println!("No command specified");
            print!("{}", HELP);
        }
    }
}

fn build(pargs: &mut pico_args::Arguments) {
    let mut cargo = Command::new("cargo")
        .arg("rustc")
        .arg("--crate-type=dylib")
        .arg("-p")
        .arg("example_lib")
        .spawn()
        .expect("Failed to spawn cargo build");

    let cargo_status = cargo.wait().expect("Failed to wait for cargo build");
    assert!(cargo_status.success(), "Cargo build failed");
}

fn run(pargs: &mut pico_args::Arguments) {
    build(pargs);

    let library_path = {
        // Canonicalize the path to the library.
        let mut path = std::env::current_dir().expect("Failed to get current dir");
        path.push("target/debug/libexample_lib.dylib");
        path
    };

    let mut lib = Command::new("target/debug/example_cli")
        .env("VERBOSE", "y")
        .env("SYMBOL", "say_hello")
        .env("PLONK_LIBRARY", library_path)
        .env("DYLD_INSERT_LIBRARIES", "../inject.dylib")
        .spawn()
        .expect("Failed to spawn");

    let lib_status = lib.wait().expect("Failed to wait for lib");
    assert!(lib_status.success(), "Lib failed");
}
