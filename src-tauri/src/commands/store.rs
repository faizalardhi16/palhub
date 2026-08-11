use tauri::State;

use crate::AppState;

#[tauri::command]
pub fn store_list(state: State<'_, AppState>) -> Result<Vec<crate::core::SkillMeta>, String> {
    state.store.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_install(
    state: State<'_, AppState>,
    source: String,
    name: Option<String>,
) -> Result<crate::core::SkillMeta, String> {
    state.store.install(&source, name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_remove(state: State<'_, AppState>, name: String) -> Result<(), String> {
    state.store.remove(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn store_refresh(
    state: State<'_, AppState>,
    name: String,
) -> Result<crate::core::SkillMeta, String> {
    state.store.refresh(&name).map_err(|e| e.to_string())
}
