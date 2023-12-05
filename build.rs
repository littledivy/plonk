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

use std::env;
use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let deps_dir = format!("{}/deps", out_dir);

    download_deps().expect("Failed to download dependencies");

    let inject_dylib = format!("{}/inject.dylib", out_dir);

    let output = Command::new("clang")
        .args(&[
            "plonk_inject.c",
            "-o",
            &inject_dylib,
            "-shared",
            &format!("-L{}", deps_dir),
            &format!("-I{}", deps_dir),
            "-lfrida-gum",
        ])
        .spawn()
        .expect("failed to execute process");

    let output = output.wait_with_output().expect("failed to wait on child");

    assert!(output.status.success());
    println!("cargo:rustc-env=PLONK_INJECT_DYLIB={}", inject_dylib);
}

fn download_deps() -> Result<(), Box<dyn std::error::Error>> {
    let arch = "arm64";
    let os = "macos";
    let version = "16.0.19";

    let devkit_name = format!("frida-gum-devkit-{}-{}-{}", version, os, arch);
    let frida_url = format!(
        "https://github.com/frida/frida/releases/download/{}/{}.tar.xz",
        version, devkit_name
    );

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set by Cargo");

    let deps_dir = Path::new(&out_dir).join("deps");

    if !deps_dir.exists() {
        fs::create_dir_all(&deps_dir)?;
    }

    let tar_path = deps_dir.join(format!("{}.tar.xz", devkit_name));

    if !tar_path.exists() {
        // Download the tarball
        let mut res = reqwest::blocking::get(&frida_url)?;
        let mut file = File::create(&tar_path)?;
        io::copy(&mut res, &mut file)?;
    }

    if tar_path.exists() {
        let output = Command::new("tar")
            .args(&["-xf", &tar_path.to_string_lossy()])
            .current_dir(&deps_dir)
            .output()?;

        if output.status.success() {
            Ok(())
        } else {
            Err(format!("Failed to extract tar file: {:?}", output).into())
        }
    } else {
        Err("Tarball not found after download.".into())
    }
}
