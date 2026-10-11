//! Wanderdex Setup: a branded front end for the Wanderdex NSIS installer, which is embedded in this
//! binary (see build.rs) and run silently. NSIS still owns files, shortcuts, the uninstaller and upgrades.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.exe"));

/// Must match the name the app's autostart plugin registers (its product name).
#[cfg(windows)]
const APP_NAME: &str = "Wanderdex";

#[derive(serde::Serialize)]
struct Defaults {
    dir: String,
    installed: bool,
}

/// Where NSIS installs per-user by default, and whether Wanderdex is already there.
#[tauri::command]
fn defaults(app: AppHandle) -> Defaults {
    let dir = app
        .path()
        .local_data_dir()
        .map(|d| d.join("Wanderdex"))
        .unwrap_or_else(|_| PathBuf::from("Wanderdex"));
    Defaults { installed: find_exe(&dir).is_some(), dir: dir.display().to_string() }
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog().file().blocking_pick_folder().map(|p| p.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Options {
    dir: String,
    desktop_shortcut: bool,
    launch_at_login: bool,
}

/// Runs the embedded installer silently, then applies the options NSIS can't take on its command line.
/// Returns the installed app's path for `launch`.
#[tauri::command]
async fn install(app: AppHandle, options: Options) -> Result<String, String> {
    let dir = PathBuf::from(&options.dir);
    tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        run_installer(&dir)?;
        let exe = find_exe(&dir).ok_or("Setup finished, but Wanderdex isn't in the install folder.")?;
        if !options.desktop_shortcut {
            // Silent NSIS installs always create one; remove it if it wasn't wanted.
            if let Ok(desktop) = app.path().desktop_dir() {
                let _ = std::fs::remove_file(desktop.join("Wanderdex.lnk"));
            }
        }
        set_autostart(&exe, options.launch_at_login)?;
        Ok(exe.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_launch_at_login(exe: String, enabled: bool) -> Result<(), String> {
    set_autostart(Path::new(&exe), enabled)
}

#[tauri::command]
fn launch(app: AppHandle, exe: String) -> Result<(), String> {
    if !exe.is_empty() {
        std::process::Command::new(&exe).spawn().map_err(|e| format!("Couldn't open Wanderdex: {e}"))?;
    }
    app.exit(0);
    Ok(())
}

/// The installed app binary: the one .exe in the folder that isn't the uninstaller.
fn find_exe(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe"))
            && !p.file_stem().is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("uninstall"))
    })
}

#[cfg(windows)]
fn run_installer(dir: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    if PAYLOAD.is_empty() {
        return Err("This build of setup doesn't contain the installer.".into());
    }
    let tmp = std::env::temp_dir().join(format!("wanderdex-install-{}.exe", std::process::id()));
    std::fs::write(&tmp, PAYLOAD).map_err(|e| format!("Couldn't unpack the installer: {e}"))?;
    // NSIS wants /D= last and unquoted, even when the path has spaces.
    let status = std::process::Command::new(&tmp)
        .arg("/S")
        .raw_arg(format!("/D={}", dir.display()))
        .status();
    let _ = std::fs::remove_file(&tmp);
    match status.map_err(|e| format!("Couldn't start the installer: {e}"))?.code() {
        Some(0) => Ok(()),
        Some(code) => Err(format!(
            "The installer stopped (code {code}). If Wanderdex is open, quit it from the tray and try again."
        )),
        None => Err("The installer was stopped.".into()),
    }
}

/// No installer to run outside Windows: pretend, so the UI can be worked on anywhere.
#[cfg(not(windows))]
fn run_installer(_dir: &Path) -> Result<(), String> {
    let _ = PAYLOAD;
    std::thread::sleep(std::time::Duration::from_secs(4));
    Ok(())
}

#[cfg(windows)]
fn set_autostart(exe: &Path, enabled: bool) -> Result<(), String> {
    let launcher = auto_launch::AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(&exe.display().to_string())
        .build()
        .map_err(|e| e.to_string())?;
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| format!("Couldn't change launch at sign-in: {e}"))
}

#[cfg(not(windows))]
fn set_autostart(_exe: &Path, _enabled: bool) -> Result<(), String> {
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![defaults, pick_folder, install, set_launch_at_login, launch])
        .run(tauri::generate_context!())
        .expect("error while running Wanderdex Setup");
}
