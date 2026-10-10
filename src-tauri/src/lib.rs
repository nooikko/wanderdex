pub mod animdata;
pub mod bounds;
pub mod brain;
pub mod commands;
pub mod ground;
pub mod index;
pub mod persist;
pub mod pets;
pub mod sprites;

use tauri::{Manager, RunEvent};

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::pet_ready,
            commands::pet_click,
            commands::pet_drag_start,
            commands::pet_drag_move,
            commands::pet_drop,
            commands::pet_context_menu,
        ])
        .setup(|app| {
            let cache_root = app.path().app_data_dir()?;
            let settings_path = app.path().app_config_dir()?.join("settings.json");
            app.manage(pets::AppState::new(cache_root, settings_path));
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            app.on_menu_event(|app, event| {
                let id = event.id().as_ref();
                if let Some(label) = id.strip_prefix("pet-move:") {
                    commands::pet_move(app, label);
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
                    if let Err(e) = pets::spawn_pet(&handle, &variant, None).await {
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
