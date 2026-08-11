use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::core::tools;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub store_dir: String,
    pub tools: HashMap<String, Option<String>>,
}

pub fn app_info_impl(state: &AppState) -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        store_dir: state.store.base_dir.display().to_string(),
        tools: tools::detect_tool_clis(),
    })
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> Result<AppInfo, String> {
    app_info_impl(state.inner())
}
