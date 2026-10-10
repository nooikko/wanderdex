pub mod animdata;
pub mod bounds;
pub mod index;
pub mod sprites;

use tauri::RunEvent;

pub fn run() {
    tauri::Builder::default()
        .build(tauri::generate_context!())
        .expect("error while building Wanderdex")
        .run(|_app, event| {
            // Wanderdex lives in the tray: closing the last window must not quit.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
