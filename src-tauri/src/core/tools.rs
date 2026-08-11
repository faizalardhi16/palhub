use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

/// Canonical tool identifiers used across the app (README §4.2).
pub const TOOLS: [&str; 4] = ["cursor", "codex", "claude-code", "opencode"];

pub fn is_valid_tool(tool: &str) -> bool {
    TOOLS.contains(&tool)
}

pub fn home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow!("cannot resolve home directory"))
}

/// Detect which coding tool CLIs are present on the machine (via PATH).
pub fn detect_tool_clis() -> HashMap<String, Option<String>> {
    let mut out = HashMap::new();
    for (tool, names) in [
        ("cursor", &["cursor", "cursor.cmd"][..]),
        ("codex", &["codex"][..]),
        ("claude-code", &["claude"][..]),
        ("opencode", &["opencode"][..]),
        ("qoder", &["qoder", "qoder.cmd"][..]),
    ] {
        out.insert(tool.to_string(), find_in_path(names));
    }
    out
}

fn find_in_path(names: &[&str]) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate.display().to_string());
            }
            // Windows PATHEXT fallback (.exe)
            if cfg!(windows) {
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe.display().to_string());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tool folder locations
// ---------------------------------------------------------------------------

pub fn global_dir(tool: &str) -> Result<PathBuf> {
    let home = home()?;
    Ok(match tool {
        "cursor" => home.join(".cursor").join("rules"),
        "codex" => home.join(".codex").join("skills"),
        "claude-code" => home.join(".claude").join("skills"),
        "opencode" => home.join(".config").join("opencode"),
        _ => bail!("unknown tool: {tool}"),
    })
}

pub fn project_dir(tool: &str, root: &Path) -> Result<PathBuf> {
    Ok(match tool {
        "cursor" => root.join(".cursor").join("rules"),
        "codex" => root.join(".codex").join("skills"),
        "claude-code" => root.join(".claude").join("skills"),
        "opencode" => root.to_path_buf(), // AGENTS.md lives at project root
        _ => bail!("unknown tool: {tool}"),
    })
}

/// Does this tool write a folder copy (native skills dir) vs AGENTS.md section?
pub fn uses_folder_copy(tool: &str) -> bool {
    matches!(tool, "codex" | "claude-code")
}

pub fn uses_mdc_rule(tool: &str) -> bool {
    tool == "cursor"
}

pub fn uses_agents_md(tool: &str) -> bool {
    tool == "opencode"
}

// ---------------------------------------------------------------------------
// Project scanning helpers
// ---------------------------------------------------------------------------

pub fn list_skill_folders(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("SKILL.md").exists())
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .collect::<Vec<_>>();
    out.sort();
    out
}

pub fn list_mdc_rules(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "mdc").unwrap_or(false))
        .filter_map(|p| {
            p.file_stem().map(|n| n.to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    out.sort();
    out
}

/// Parse PalHub-managed sections from an AGENTS.md (`<!-- palhub:<name> -->`).
pub fn list_agents_md_sections(path: &Path) -> Vec<String> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut out = Vec::new();
    for line in raw.lines() {
        if let Some(inner) = line.trim().strip_prefix("<!-- palhub:") {
            if let Some(name) = inner.strip_suffix("-->") {
                let name = name.trim();
                if !name.is_empty() {
                    out.push(name.to_string());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
