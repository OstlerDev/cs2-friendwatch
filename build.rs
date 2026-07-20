//! Bundle Steam redistributables into the binary and enable delay-load on MSVC
//! so a single `.exe` can ship without a sidecar DLL.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=steam_appid.txt");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let lib_name = steam_api_lib_name();

    let dll_src = find_steam_api_dll(lib_name).unwrap_or_else(|| {
        panic!(
            "could not find {lib_name} under target/*/build/steamworks-sys-*/out — \
             build steamworks-sys first (cargo build)"
        )
    });

    let dll_dst = out_dir.join(lib_name);
    fs::copy(&dll_src, &dll_dst).unwrap_or_else(|e| {
        panic!("failed to copy {} → {}: {e}", dll_src.display(), dll_dst.display())
    });
    println!("cargo:rerun-if-changed={}", dll_src.display());

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let appid_src = manifest.join("steam_appid.txt");
    fs::copy(&appid_src, out_dir.join("steam_appid.txt")).unwrap_or_else(|e| {
        panic!("failed to copy steam_appid.txt: {e}")
    });

    // Delay-load so the process can start before the DLL is extracted at runtime.
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") {
        println!("cargo:rustc-link-arg=/DELAYLOAD:{lib_name}");
        println!("cargo:rustc-link-lib=delayimp");
    }

    // Convenience for `cargo run`: also place redistributables next to the built exe.
    if let Some(profile_dir) = out_dir.ancestors().nth(3).map(Path::to_path_buf) {
        let _ = fs::copy(&dll_dst, profile_dir.join(lib_name));
        let _ = fs::copy(&appid_src, profile_dir.join("steam_appid.txt"));
    }
}

fn find_steam_api_dll(lib_name: &str) -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").ok()?);
    // target/<profile>/build/<crate>/out → climb to target/<profile>
    let profile_dir = out_dir.ancestors().nth(3)?;
    let build_dir = profile_dir.join("build");
    let entries = fs::read_dir(build_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("steamworks-sys-") {
            continue;
        }
        let candidate = entry.path().join("out").join(lib_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn steam_api_lib_name() -> &'static str {
    let target = env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        if target.contains("i686") {
            "steam_api.dll"
        } else {
            "steam_api64.dll"
        }
    } else if target.contains("darwin") || target.contains("apple") {
        "libsteam_api.dylib"
    } else {
        "libsteam_api.so"
    }
}
