//! Menu-bar / tray icon and its menu; opens the picker and credits windows.

use crate::pets::{self, AppState};
use crate::persist::SCALES;
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;

const TRAY_ID: &str = "main";

fn menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let (scale, hidden) = {
        let s = app.state::<AppState>().settings.lock().unwrap().clone();
        (s.scale, s.hidden)
    };
    let mut size = SubmenuBuilder::new(app, "Size");
    for s in SCALES {
        size = size.item(&CheckMenuItemBuilder::with_id(format!("size-{s}"), format!("{s}×")).checked(s == scale).build(app)?);
    }
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id("open-picker", "Open picker").build(app)?)
        .item(&size.build()?)
        .item(&MenuItemBuilder::with_id("toggle-hidden", if hidden { "Show all" } else { "Hide all" }).build(app)?)
        .item(&CheckMenuItemBuilder::with_id("autostart", "Launch at login").checked(autostart).build(app)?)
        .item(&MenuItemBuilder::with_id("credits", "Credits && license").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("quit", "Quit Wanderdex").build(app)?)
        .build()
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("bundle icon").clone())
        .tooltip("Wanderdex")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .build(app)?;
    Ok(())
}

pub fn refresh_tray(app: &AppHandle) {
    if let (Some(tray), Ok(m)) = (app.tray_by_id(TRAY_ID), menu(app)) {
        let _ = tray.set_menu(Some(m));
    }
}

fn open_window(app: &AppHandle, label: &str, page: &str, title: &str, size: (f64, f64)) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, label, WebviewUrl::App(page.into()))
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(640.0, 480.0)
        .center()
        .focused(true)
        .build();
}

pub fn open_picker(app: &AppHandle) {
    open_window(app, "picker", "index.html", "Wanderdex", (980.0, 700.0));
}

pub fn open_credits(app: &AppHandle) {
    open_window(app, "credits", "credits.html", "Wanderdex Credits", (640.0, 600.0));
}

/// Handles tray and shared menu ids. Returns false for ids it doesn't own.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    match id {
        "open-picker" => open_picker(app),
        "credits" => open_credits(app),
        "toggle-hidden" => {
            let hidden = app.state::<AppState>().settings.lock().unwrap().hidden;
            pets::set_hidden(app, !hidden);
            refresh_tray(app);
        }
        "autostart" => {
            let al = app.autolaunch();
            let _ = if al.is_enabled().unwrap_or(false) { al.disable() } else { al.enable() };
            refresh_tray(app);
        }
        "quit" => {
            pets::save_settings(app);
            app.exit(0);
        }
        _ => match id.strip_prefix("size-").and_then(|s| s.parse::<u32>().ok()).filter(|s| SCALES.contains(s)) {
            Some(scale) => {
                pets::set_scale(app, scale);
                refresh_tray(app);
            }
            None => return false,
        },
    }
    true
}
