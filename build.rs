use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=steam_appid.txt");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    // OUT_DIR: target/<profile>/build/<crate>-<hash>/out → profile dir is nth(3)
    let Some(profile_dir) = out_dir.ancestors().nth(3).map(Path::to_path_buf) else {
        return;
    };

    copy_steam_api(&profile_dir);
    copy_appid(&profile_dir);
}

fn copy_steam_api(profile_dir: &Path) {
    let lib_name = steam_api_lib_name();
    let build_dir = profile_dir.join("build");
    let Some(src) = find_in_build(&build_dir, "steamworks-sys-", lib_name) else {
        println!(
            "cargo:warning=could not find {lib_name} under {}; place it next to the .exe manually",
            build_dir.display()
        );
        return;
    };
    let dst = profile_dir.join(lib_name);
    if src != dst {
        if let Err(e) = fs::copy(&src, &dst) {
            println!(
                "cargo:warning=failed to copy {} → {}: {e}",
                src.display(),
                dst.display()
            );
        }
    }
}

fn copy_appid(profile_dir: &Path) {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src = manifest.join("steam_appid.txt");
    if !src.exists() {
        return;
    }
    let dst = profile_dir.join("steam_appid.txt");
    let _ = fs::copy(&src, &dst);
}

fn find_in_build(build_dir: &Path, prefix: &str, file_name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(build_dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(prefix) {
            continue;
        }
        let candidate = entry.path().join("out").join(file_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn steam_api_lib_name() -> &'static str {
    if cfg!(all(windows, target_pointer_width = "64")) {
        "steam_api64.dll"
    } else if cfg!(windows) {
        "steam_api.dll"
    } else if cfg!(target_os = "macos") {
        "libsteam_api.dylib"
    } else {
        "libsteam_api.so"
    }
}
