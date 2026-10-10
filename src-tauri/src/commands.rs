//! Tauri commands called from the pet, picker, credits and contact windows.

use crate::bounds::BoxI;
use crate::brain::Visual;
use crate::pets::{self, AppState};
use crate::animdata::AnimInfo;
use serde::Serialize;
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInit {
    pub dir: String,
    pub table: Vec<AnimInfo>,
    pub content_box: BoxI,
    pub scale: u32,
    pub visual: Visual,
}

fn with_pet<T>(window: &WebviewWindow, f: impl FnOnce(&mut pets::Pet) -> T) -> Option<T> {
    let app = window.app_handle();
    let state = app.state::<AppState>();
    let mut pets = state.pets.lock().unwrap();
    pets.iter_mut().find(|p| p.label == window.label()).map(f)
}

fn cursor(window: &WebviewWindow) -> (f64, f64) {
    window.app_handle().cursor_position().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))
}

#[tauri::command]
pub async fn pet_ready(window: WebviewWindow) -> Result<PetInit, String> {
    let scale = window.app_handle().state::<AppState>().settings.lock().unwrap().scale;
    with_pet(&window, |p| PetInit {
        dir: p.dir.to_string_lossy().into_owned(),
        table: p.table.anims.clone(),
        content_box: p.content,
        scale,
        visual: p.brain.visual(),
    })
    .ok_or_else(|| "pet not found".to_string())
}

#[tauri::command]
pub async fn pet_click(window: WebviewWindow) {
    with_pet(&window, |p| p.brain.click());
}

#[tauri::command]
pub async fn pet_drag_start(window: WebviewWindow) {
    let c = cursor(&window);
    with_pet(&window, |p| p.brain.drag_start(c));
}

#[tauri::command]
pub async fn pet_drag_move(window: WebviewWindow) {
    let c = cursor(&window);
    with_pet(&window, |p| p.brain.drag_move(c));
}

#[tauri::command]
pub async fn pet_drop(window: WebviewWindow) {
    with_pet(&window, |p| p.brain.drop_pet());
    pets::save_settings(window.app_handle());
}

#[tauri::command]
pub async fn pet_context_menu(window: WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    let label = window.label();
    let menu = MenuBuilder::new(app)
        .item(&MenuItemBuilder::with_id(format!("pet-move:{label}"), "Do a move").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id(format!("pet-credits:{label}"), "Credits for this Pokémon").build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id(format!("pet-remove:{label}"), "Remove").build(app).map_err(|e| e.to_string())?)
        .item(&MenuItemBuilder::with_id("pets-remove-all", "Remove all").build(app).map_err(|e| e.to_string())?)
        .separator()
        .item(&MenuItemBuilder::with_id("open-picker", "Open picker").build(app).map_err(|e| e.to_string())?)
        .build()
        .map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// Shared by menus: make a pet do its move.
pub fn pet_move(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    let mut pets = state.pets.lock().unwrap();
    if let Some(p) = pets.iter_mut().find(|p| p.label == label) {
        p.brain.click();
    }
}
