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
    pets::cursor(window.app_handle())
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

use crate::index::{bundled, find_variant, signature_for, Index};
use crate::sprites::{ensure_portrait, ensure_variant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantPayload {
    pub dir: String,
    pub table: Vec<AnimInfo>,
    pub content_box: BoxI,
    pub signature: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePet {
    pub label: String,
    pub variant: String,
    pub dex: String,
    pub name: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
}

#[tauri::command]
pub fn get_index() -> Index {
    bundled().clone()
}

#[tauri::command]
pub async fn ensure_portrait_cmd(app: AppHandle, dex: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let path = ensure_portrait(&state.http, &state.cache_root, &bundled().sprite_collab_sha, &dex)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn load_variant(app: AppHandle, path: String) -> Result<VariantPayload, String> {
    let (species, _) = find_variant(bundled(), &path).ok_or_else(|| format!("Unknown variant {path}"))?;
    let signature = signature_for(&species.dex).map(String::from);
    let state = app.state::<AppState>();
    let v = ensure_variant(&state.http, &state.cache_root, &bundled().sprite_collab_sha, &path)
        .await
        .map_err(|e| e.to_string())?;
    let content_box = crate::bounds::content_box(&v.table, &v.sheets, &crate::brain::behavior_anims(signature.as_deref()));
    Ok(VariantPayload { dir: v.dir.to_string_lossy().into_owned(), table: v.table.anims, content_box, signature })
}

#[tauri::command]
pub async fn spawn(app: AppHandle, variant: String) -> Result<(), String> {
    pets::spawn_pet(&app, &variant, None).await
}

#[tauri::command]
pub fn list_pets(app: AppHandle) -> Vec<ActivePet> {
    let state = app.state::<AppState>();
    let pets = state.pets.lock().unwrap();
    pets.iter()
        .filter_map(|p| {
            let (s, v) = find_variant(bundled(), &p.variant)?;
            Some(ActivePet {
                label: p.label.clone(),
                variant: p.variant.clone(),
                dex: s.dex.clone(),
                name: s.name.clone(),
                form: v.form.clone(),
                shiny: v.shiny,
                female: v.female,
            })
        })
        .collect()
}

#[tauri::command]
pub fn remove_pet_cmd(app: AppHandle, label: String) {
    pets::remove_pet(&app, &label);
}

#[tauri::command]
pub fn remove_all_cmd(app: AppHandle) {
    pets::remove_all(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditEntry {
    pub name: String,
    pub form: String,
    pub shiny: bool,
    pub female: bool,
    pub credits: Vec<String>,
}

#[tauri::command]
pub fn get_credits(app: AppHandle) -> Vec<CreditEntry> {
    let credited = app.state::<AppState>().settings.lock().unwrap().credited.clone();
    credited
        .iter()
        .filter_map(|path| {
            let (s, v) = find_variant(bundled(), path)?;
            Some(CreditEntry { name: s.name.clone(), form: v.form.clone(), shiny: v.shiny, female: v.female, credits: v.credits.clone() })
        })
        .collect()
}

#[tauri::command]
pub fn save_contact_sheet(app: AppHandle, data_url: String, report: String, ok: bool) -> Result<(), String> {
    use std::io::Write;
    let b64 = data_url.split_once(',').map(|(_, b)| b).ok_or("bad data URL")?;
    let bytes = base64_decode(b64)?;
    std::fs::write("contact-sheet.png", bytes).map_err(|e| e.to_string())?;
    println!("{report}");
    let _ = std::io::stdout().flush();
    app.exit(if ok { 0 } else { 1 });
    Ok(())
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| *c != b'=') {
        let v = ALPHABET.iter().position(|a| *a == c).ok_or("bad base64")? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}
