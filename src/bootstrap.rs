//! Extract the embedded Steam redistributable before any Steamworks call.
//!
//! On Windows MSVC the binary delay-loads `steam_api64.dll`, so the process can
//! start from a single `.exe`. We unpack the DLL (and app id) into local app
//! data and point the loader at that folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(all(windows, target_pointer_width = "64"))]
const STEAM_API_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/steam_api64.dll"));
#[cfg(all(windows, target_pointer_width = "64"))]
const STEAM_API_NAME: &str = "steam_api64.dll";

#[cfg(all(windows, not(target_pointer_width = "64")))]
const STEAM_API_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/steam_api.dll"));
#[cfg(all(windows, not(target_pointer_width = "64")))]
const STEAM_API_NAME: &str = "steam_api.dll";

#[cfg(target_os = "macos")]
const STEAM_API_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libsteam_api.dylib"));
#[cfg(target_os = "macos")]
const STEAM_API_NAME: &str = "libsteam_api.dylib";

#[cfg(all(unix, not(target_os = "macos")))]
const STEAM_API_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libsteam_api.so"));
#[cfg(all(unix, not(target_os = "macos")))]
const STEAM_API_NAME: &str = "libsteam_api.so";

const STEAM_APPID_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/steam_appid.txt"));

/// Unpack Steam runtime files and configure the process so Steamworks can load.
pub fn ensure_steam_runtime() -> Result<(), String> {
    let dir = runtime_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;

    write_if_changed(&dir.join(STEAM_API_NAME), STEAM_API_BYTES)
        .map_err(|e| format!("write {STEAM_API_NAME}: {e}"))?;
    write_if_changed(&dir.join("steam_appid.txt"), STEAM_APPID_BYTES)
        .map_err(|e| format!("write steam_appid.txt: {e}"))?;

    let appid = std::str::from_utf8(STEAM_APPID_BYTES)
        .unwrap_or("480")
        .lines()
        .next()
        .unwrap_or("480")
        .trim();
    // Prefered over relying on cwd for steam_appid.txt.
    // Safety: called once at process start before other threads touch env.
    unsafe { std::env::set_var("SteamAppId", appid) };

    #[cfg(windows)]
    set_dll_directory(&dir)?;

    #[cfg(target_os = "linux")]
    prepend_ld_library_path(&dir)?;

    #[cfg(target_os = "macos")]
    prepend_dyld_path(&dir)?;

    Ok(())
}

fn runtime_dir() -> Result<PathBuf, String> {
    let mut dir = dirs::data_local_dir().ok_or_else(|| "could not resolve local data dir".to_string())?;
    dir.push("cs2-friendwatch");
    dir.push("steam");
    Ok(dir)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if path.exists() {
        if let Ok(existing) = fs::read(path) {
            if existing == bytes {
                return Ok(());
            }
        }
    }
    fs::write(path, bytes)
}

#[cfg(windows)]
fn set_dll_directory(dir: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetDllDirectoryW(path: *const u16) -> i32;
    }

    let ok = unsafe { SetDllDirectoryW(wide.as_ptr()) };
    if ok == 0 {
        return Err(format!(
            "SetDllDirectoryW failed for {}",
            dir.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn prepend_ld_library_path(dir: &Path) -> Result<(), String> {
    let dir_s = dir.to_string_lossy();
    let merged = match std::env::var_os("LD_LIBRARY_PATH") {
        Some(existing) => {
            let mut v = dir_s.into_owned();
            v.push(':');
            v.push_str(&existing.to_string_lossy());
            v
        }
        None => dir_s.into_owned(),
    };
    unsafe { std::env::set_var("LD_LIBRARY_PATH", merged) };
    Ok(())
}

#[cfg(target_os = "macos")]
fn prepend_dyld_path(dir: &Path) -> Result<(), String> {
    let dir_s = dir.to_string_lossy();
    let merged = match std::env::var_os("DYLD_LIBRARY_PATH") {
        Some(existing) => {
            let mut v = dir_s.into_owned();
            v.push(':');
            v.push_str(&existing.to_string_lossy());
            v
        }
        None => dir_s.into_owned(),
    };
    unsafe { std::env::set_var("DYLD_LIBRARY_PATH", merged) };
    Ok(())
}
