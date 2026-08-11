use std::collections::HashMap;
use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::core::tools;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub detected_tools: Vec<String>,
    pub has_package_json: bool,
    pub has_git: bool,
    pub has_agents_md: bool,
    pub has_claude_md: bool,
    pub injected: HashMap<String, Vec<String>>,
}

#[tauri::command]
pub fn project_open(path: String) -> Result<ProjectInfo, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string());

    let mut detected_tools = Vec::new();
    if root.join(".cursor").is_dir() {
        detected_tools.push("cursor".to_string());
    }
    if root.join(".codex").is_dir() {
        detected_tools.push("codex".to_string());
    }
    if root.join(".claude").is_dir() {
        detected_tools.push("claude-code".to_string());
    }
    if root.join("opencode.json").is_file() {
        detected_tools.push("opencode".to_string());
    }

    let mut injected: HashMap<String, Vec<String>> = HashMap::new();
    injected.insert(
        "cursor".to_string(),
        tools::list_mdc_rules(&root.join(".cursor").join("rules")),
    );
    injected.insert(
        "codex".to_string(),
        tools::list_skill_folders(&root.join(".codex").join("skills")),
    );
    injected.insert(
        "claude-code".to_string(),
        tools::list_skill_folders(&root.join(".claude").join("skills")),
    );
    injected.insert(
        "opencode".to_string(),
        tools::list_agents_md_sections(&root.join("AGENTS.md")),
    );

    Ok(ProjectInfo {
        path: root.display().to_string(),
        name,
        detected_tools,
        has_package_json: root.join("package.json").is_file(),
        has_git: root.join(".git").exists(),
        has_agents_md: root.join("AGENTS.md").is_file(),
        has_claude_md: root.join("CLAUDE.md").is_file(),
        injected,
    })
}

#[tauri::command]
pub fn project_inject(
    state: State<'_, AppState>,
    tool: String,
    skill: String,
    scope: String,
    path: Option<String>,
) -> Result<crate::core::injector::InjectResult, String> {
    let root = path.map(PathBuf::from);
    let inj = crate::core::injector::Injector::new(&state.store);
    inj.inject(&tool, &skill, &scope, root.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn project_uninject(
    state: State<'_, AppState>,
    tool: String,
    skill: String,
    scope: String,
    path: Option<String>,
) -> Result<crate::core::injector::InjectResult, String> {
    let root = path.map(PathBuf::from);
    let inj = crate::core::injector::Injector::new(&state.store);
    inj.uninject(&tool, &skill, &scope, root.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn app_open_folder(path: String) -> Result<(), String> {
    open_in_explorer(&path).map_err(|e| e.to_string())
}

fn open_in_explorer(path: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
    }
}
