pub mod commands;
pub mod core;

use std::collections::HashMap;
use std::sync::Mutex;

use commands::terminal::SessionHandle;
use core::skill_store::SkillStore;

/// Shared application state.
pub struct AppState {
    pub store: SkillStore,
    pub sessions: Mutex<HashMap<String, SessionHandle>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let store = SkillStore::new().expect("cannot initialize ~/.palhub skill store");
    let state = AppState {
        store,
        sessions: Mutex::new(HashMap::new()),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::store::store_list,
            commands::store::store_install,
            commands::store::store_remove,
            commands::store::store_refresh,
            commands::project::project_open,
            commands::project::project_inject,
            commands::project::project_uninject,
            commands::terminal::terminal_run,
            commands::terminal::terminal_kill,
            commands::terminal::terminal_list,
            commands::app::app_info,
            commands::project::app_open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PalHub");
}
