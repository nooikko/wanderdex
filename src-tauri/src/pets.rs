//! Live pets: one transparent window each, moved by a 60 Hz loop that ticks every pet's brain.

use crate::bounds::{content_box, feet_line, BoxI};
use crate::brain::{behavior_anims, AnimSet, Brain, Visual};
use crate::ground::{ground_for, Ground, MonitorInfo};
use crate::index::{bundled, find_variant, signature_for};
use crate::persist::{self, SavedPet, Settings};
use crate::sprites::{ensure_variant, HttpFetch};
use crate::animdata::AnimTable;
use rand::RngExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub struct Pet {
    pub label: String,
    pub variant: String,
    pub dir: PathBuf,
    pub table: AnimTable,
    pub content: BoxI,
    /// Standing foot line (sprite px below the frame center); rests on the ground.
    pub feet: i32,
    pub brain: Brain,
    pub window: WebviewWindow,
    last_visual: Option<Visual>,
    last_ppt: f64,
    last_pos: (i32, i32),
    shown: bool,
}

pub struct AppState {
    pub pets: Mutex<Vec<Pet>>,
    pub settings: Mutex<Settings>,
    pub cache_root: PathBuf,
    pub settings_path: PathBuf,
    pub http: HttpFetch,
    pub next_id: AtomicU64,
    monitors: Mutex<Vec<MonitorInfo>>,
}

impl AppState {
    pub fn new(cache_root: PathBuf, settings_path: PathBuf) -> Self {
        let settings = persist::load(&settings_path);
        AppState {
            pets: Mutex::new(Vec::new()),
            settings: Mutex::new(settings),
            cache_root,
            settings_path,
            http: HttpFetch::new(),
            next_id: AtomicU64::new(1),
            monitors: Mutex::new(Vec::new()),
        }
    }
}

pub fn monitor_infos(app: &AppHandle) -> Vec<MonitorInfo> {
    let primary = app.primary_monitor().ok().flatten().map(|m| *m.position());
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let wa = m.work_area();
            MonitorInfo {
                x: wa.position.x as f64,
                y: wa.position.y as f64,
                width: wa.size.width as f64,
                height: wa.size.height as f64,
                scale: m.scale_factor(),
                primary: primary == Some(*m.position()),
            }
        })
        .collect()
}

fn ground_at(app: &AppHandle, x: f64, y: f64) -> Ground {
    let state = app.state::<AppState>();
    let mut monitors = state.monitors.lock().unwrap();
    if monitors.is_empty() {
        *monitors = monitor_infos(app);
    }
    ground_for(x, y, &monitors).unwrap_or(Ground { left: 0.0, right: 1280.0, y: 800.0, px_per_pt: 1.0 })
}

fn cursor(app: &AppHandle) -> (f64, f64) {
    app.cursor_position().map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))
}

fn window_size(content: &BoxI, scale: u32, ppt: f64) -> PhysicalSize<u32> {
    let k = scale as f64 * ppt;
    PhysicalSize::new((content.width() as f64 * k).round() as u32, (content.height() as f64 * k).round() as u32)
}

/// Downloads (if needed) and spawns a pet. `at` is a physical (x, feet y); default: random spot under the cursor.
pub async fn spawn_pet(app: &AppHandle, variant: &str, at: Option<(f64, f64)>) -> Result<(), String> {
    let index = bundled();
    let (species, _) = find_variant(index, variant).ok_or_else(|| format!("Unknown variant {variant}"))?;
    let state = app.state::<AppState>();
    let loaded = ensure_variant(&state.http, &state.cache_root, &index.sprite_collab_sha, variant)
        .await
        .map_err(|e| e.to_string())?;
    let signature = signature_for(&species.dex).map(String::from);
    let content = content_box(&loaded.table, &loaded.sheets, &behavior_anims(signature.as_deref()));
    let feet = feet_line(&loaded.table, &loaded.sheets);
    let scale = state.settings.lock().unwrap().scale;

    let (x, y) = at.unwrap_or_else(|| {
        let c = cursor(app);
        let g = ground_at(app, c.0, c.1);
        (rand::rng().random_range(g.left..g.right.max(g.left + 1.0)), g.y)
    });
    let g = ground_at(app, x, y);
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let label = format!("pet-{id}");
    let pts = (content.width() as f64 * scale as f64, content.height() as f64 * scale as f64);
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("pet.html".into()))
        .title("Wanderdex pet")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(false)
        .inner_size(pts.0, pts.1)
        .build()
        .map_err(|e| e.to_string())?;
    let mut brain = Brain::new(id ^ 0x5eed, AnimSet::from_table(&loaded.table), signature, scale, (-content.min_x as f64, content.max_x as f64), x, &g);
    brain.y = y.min(g.y);
    if brain.y < g.y {
        brain.drag_start((brain.x, brain.y));
        brain.drop_pet(); // restored mid-air: fall to the ground
    }
    let pet = Pet {
        label,
        variant: variant.to_string(),
        dir: loaded.dir,
        table: loaded.table,
        content,
        feet,
        brain,
        window,
        last_visual: None,
        last_ppt: 0.0,
        last_pos: (i32::MIN, i32::MIN),
        shown: false,
    };
    state.pets.lock().unwrap().push(pet);
    {
        let mut s = state.settings.lock().unwrap();
        if !s.credited.iter().any(|c| c == variant) {
            s.credited.push(variant.to_string());
        }
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
    Ok(())
}

pub fn remove_pet(app: &AppHandle, label: &str) {
    let state = app.state::<AppState>();
    let removed: Vec<Pet> = {
        let mut pets = state.pets.lock().unwrap();
        let (gone, keep): (Vec<Pet>, Vec<Pet>) = pets.drain(..).partition(|p| p.label == label);
        *pets = keep;
        gone
    };
    for p in removed {
        let _ = p.window.destroy();
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
}

pub fn remove_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let all: Vec<Pet> = state.pets.lock().unwrap().drain(..).collect();
    for p in all {
        let _ = p.window.destroy();
    }
    save_settings(app);
    let _ = app.emit("pets:changed", ());
}

pub fn set_scale(app: &AppHandle, scale: u32) {
    let state = app.state::<AppState>();
    state.settings.lock().unwrap().scale = scale;
    for p in state.pets.lock().unwrap().iter_mut() {
        p.brain.set_scale(scale);
        p.last_ppt = 0.0; // force a resize on the next tick
        let _ = app.emit_to(p.label.as_str(), "pet:scale", scale);
    }
    save_settings(app);
}

pub fn set_hidden(app: &AppHandle, hidden: bool) {
    let state = app.state::<AppState>();
    state.settings.lock().unwrap().hidden = hidden;
    for p in state.pets.lock().unwrap().iter() {
        let _ = if hidden { p.window.hide() } else { p.window.show() };
    }
    save_settings(app);
}

/// Writes settings, including every live pet's position.
pub fn save_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let pets: Vec<SavedPet> = state
        .pets
        .lock()
        .unwrap()
        .iter()
        .map(|p| SavedPet { variant: p.variant.clone(), x: p.brain.x, y: p.brain.y })
        .collect();
    let snapshot = {
        let mut s = state.settings.lock().unwrap();
        s.pets = pets;
        s.clone()
    };
    if let Err(e) = persist::save(&state.settings_path, &snapshot) {
        eprintln!("Couldn't save settings: {e}");
    }
}

/// Top-left of a pet window whose anchor is at x and whose foot line rests on ground y.
pub fn window_origin(x: f64, y: f64, content: &BoxI, feet: i32, k: f64) -> (i32, i32) {
    let left = (x - (-content.min_x) as f64 * k).round() as i32;
    let top = (y - (feet - content.min_y) as f64 * k).round() as i32;
    (left, top)
}

fn step(app: &AppHandle, pet: &mut Pet, dt: f64, monitors: &[MonitorInfo], scale: u32, hidden: bool) {
    let g = ground_for(pet.brain.x, pet.brain.y, monitors).unwrap_or(Ground { left: 0.0, right: 1280.0, y: 800.0, px_per_pt: 1.0 });
    pet.brain.tick(dt, &g);
    let k = scale as f64 * g.px_per_pt;
    if (g.px_per_pt - pet.last_ppt).abs() > 1e-9 {
        let _ = pet.window.set_size(window_size(&pet.content, scale, g.px_per_pt));
        pet.last_ppt = g.px_per_pt;
    }
    let (left, top) = window_origin(pet.brain.x, pet.brain.y, &pet.content, pet.feet, k);
    if (left, top) != pet.last_pos {
        let _ = pet.window.set_position(PhysicalPosition::new(left, top));
        pet.last_pos = (left, top);
    }
    if !pet.shown && !hidden {
        let _ = pet.window.show();
        pet.shown = true;
    }
    let v = pet.brain.visual();
    if pet.last_visual.as_ref() != Some(&v) {
        let _ = app.emit_to(pet.label.as_str(), "pet:anim", &v);
        pet.last_visual = Some(v);
    }
}

pub fn start_loop(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = Instant::now();
        let mut last_monitor_refresh = Instant::now() - Duration::from_secs(10);
        let mut last_save = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(16));
            let now = Instant::now();
            let dt = (now - last).as_secs_f64().min(0.1);
            last = now;
            let state = app.state::<AppState>();
            if now - last_monitor_refresh > Duration::from_secs(1) {
                *state.monitors.lock().unwrap() = monitor_infos(&app);
                last_monitor_refresh = now;
            }
            let monitors = state.monitors.lock().unwrap().clone();
            let (scale, hidden) = {
                let s = state.settings.lock().unwrap();
                (s.scale, s.hidden)
            };
            for pet in state.pets.lock().unwrap().iter_mut() {
                step(&app, pet, dt, &monitors, scale, hidden);
            }
            if now - last_save > Duration::from_secs(5) {
                save_settings(&app);
                last_save = now;
            }
        }
    });
}

/// Respawns saved pets that still exist in the index.
pub fn restore(app: AppHandle) {
    let saved: Vec<SavedPet> = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        persist::restorable(&s, bundled()).into_iter().cloned().collect()
    };
    tauri::async_runtime::spawn(async move {
        for p in saved {
            if let Err(e) = spawn_pet(&app, &p.variant, Some((p.x, p.y))).await {
                eprintln!("Couldn't restore {}: {e}", p.variant);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feet_line_sits_on_the_ground_even_if_content_extends_lower() {
        // Wobbuffet: feet at 9, but a flourish reaches 12 below centre; scale 3, 1 px/pt.
        let content = BoxI { min_x: -12, min_y: -20, max_x: 12, max_y: 12 };
        let (left, top) = window_origin(500.0, 800.0, &content, 9, 3.0);
        assert_eq!(left, 500 - 36);
        assert_eq!(top, 800 - (9 + 20) * 3, "feet line (9) lands exactly on ground y");
    }
}
