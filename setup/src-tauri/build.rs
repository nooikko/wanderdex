//! Embeds the Wanderdex NSIS installer named by WANDERDEX_INSTALLER. Without it (local UI work),
//! an empty payload is embedded and the install step is simulated on non-Windows platforms.
use std::{env, fs, path::PathBuf};

fn main() {
    let payload = PathBuf::from(env::var("OUT_DIR").unwrap()).join("payload.exe");
    println!("cargo:rerun-if-env-changed=WANDERDEX_INSTALLER");
    match env::var("WANDERDEX_INSTALLER") {
        Ok(path) => {
            println!("cargo:rerun-if-changed={path}");
            fs::copy(&path, &payload).unwrap_or_else(|e| panic!("WANDERDEX_INSTALLER={path}: {e}"));
        }
        Err(_) => fs::write(&payload, b"").unwrap(),
    }
    tauri_build::build();
}
