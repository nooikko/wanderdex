pub mod animdata;
pub mod bounds;
pub mod brain;
pub mod commands;
pub mod ground;
pub mod index;
pub mod persist;
pub mod pets;
pub mod sprites;
pub mod tray;

use tauri::{Manager, RunEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            commands::pet_ready,
            commands::pet_click,
            commands::pet_drag_start,
            commands::pet_drag_move,
            commands::pet_drop,
            commands::pet_context_menu,
            commands::get_index,
            commands::ensure_portrait_cmd,
            commands::load_variant,
            commands::spawn,
            commands::list_pets,
            commands::remove_pet_cmd,
            commands::remove_all_cmd,
            commands::get_credits,
            commands::save_contact_sheet,
        ])
        .setup(|app| {
            let cache_root = app.path().app_data_dir()?;
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            app.manage(pets::AppState::new(cache_root, settings_path));
            let args: Vec<String> = std::env::args().collect();
            if let Some(i) = args.iter().position(|a| a == "--contact") {
                let variant = args.get(i + 1).cloned().unwrap_or_else(|| "0202".into());
                tauri::WebviewWindowBuilder::new(app, "contact", tauri::WebviewUrl::App(format!("contact.html?variant={variant}").into()))
                    .title("Contact sheet")
                    .inner_size(1400.0, 900.0)
                    .build()?;
                return Ok(());
            }
            tray::build_tray(app.handle())?;
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                if tray::handle_menu(app, id) {
                    return;
                }
                if let Some(label) = id.strip_prefix("pet-move:") {
                    commands::pet_move(app, label);
                } else if id.starts_with("pet-credits:") {
                    tray::open_credits(app);
                } else if let Some(label) = id.strip_prefix("pet-remove:") {
                    pets::remove_pet(app, label);
                } else if id == "pets-remove-all" {
                    pets::remove_all(app);
                }
            });
            pets::start_loop(app.handle().clone());
            pets::restore(app.handle().clone());
            // Dev convenience until the picker exists: WANDERDEX_SPAWN=0202 pnpm tauri dev
            if let Ok(variant) = std::env::var("WANDERDEX_SPAWN") {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = pets::spawn_pet(&handle, &variant).await {
                        eprintln!("spawn failed: {e}");
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Wanderdex")
        .run(|app, event| match event {
            RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
            RunEvent::Exit => pets::save_settings(app),
            _ => {}
        });
}
