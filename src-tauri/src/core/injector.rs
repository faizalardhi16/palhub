use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use super::skill_store::{skill_body, SkillStore};
use super::tools;

#[derive(Debug, Clone, Serialize)]
pub struct InjectResult {
    pub tool: String,
    pub skill: String,
    pub scope: String,
    pub target: String,
    pub status: String, // injected | updated | skipped
    pub message: String,
}

const AGENTS_MARKER: &str = "<!-- palhub:";

pub struct Injector<'a> {
    store: &'a SkillStore,
}

impl<'a> Injector<'a> {
    pub fn new(store: &'a SkillStore) -> Self {
        Self { store }
    }

    /// Inject `skill` into `tool` at `scope` (global or project under `root`).
    pub fn inject(&self, tool: &str, skill: &str, scope: &str, root: Option<&Path>) -> Result<InjectResult> {
        if !tools::is_valid_tool(tool) {
            bail!("unknown tool: {tool}");
        }
        if !matches!(scope, "global" | "project") {
            bail!("scope must be 'global' or 'project'");
        }
        if scope == "project" {
            let root = root.ok_or_else(|| anyhow!("project scope requires a root path"))?;
            if !root.is_dir() {
                bail!("project root does not exist: {}", root.display());
            }
        }

        let skill_dir = self.store.resolve(skill)?;
        let body = skill_body(&skill_dir).context("cannot read SKILL.md body")?;

        let (target_dir, target_file) = match tool {
            "cursor" => {
                let dir = self.scope_dir(tool, scope, root)?;
                let file = dir.join(format!("{skill}.mdc"));
                (dir, file)
            }
            "codex" | "claude-code" => {
                let dir = self.scope_dir(tool, scope, root)?.join(skill);
                let _ = fs::remove_dir_all(&dir);
                copy_dir(&skill_dir, &dir)?;
                (dir.clone(), dir)
            }
            "opencode" => {
                let dir = self.scope_dir(tool, scope, root)?;
                let file = dir.join("AGENTS.md");
                (dir, file)
            }
            _ => bail!("unknown tool: {tool}"),
        };

        fs::create_dir_all(&target_dir)
            .with_context(|| format!("cannot create {}", target_dir.display()))?;

        let existed = target_file.exists();
        match tool {
            "cursor" => {
                fs::write(&target_file, mdc_content(&body)).context("cannot write .mdc rule")?
            }
            "opencode" => append_agents_section(&target_file, skill, &body)?,
            _ => {}
        }
        let status = if existed { "updated" } else { "injected" };
        Ok(InjectResult {
            tool: tool.to_string(),
            skill: skill.to_string(),
            scope: scope.to_string(),
            target: target_file.display().to_string(),
            status: status.to_string(),
            message: format!(
                "{} → {} ({})",
                skill,
                target_file.display(),
                if existed { "updated" } else { "injected" }
            ),
        })
    }

    /// Remove an injected skill.
    pub fn uninject(&self, tool: &str, skill: &str, scope: &str, root: Option<&Path>) -> Result<InjectResult> {
        if !tools::is_valid_tool(tool) {
            bail!("unknown tool: {tool}");
        }
        if !matches!(scope, "global" | "project") {
            bail!("scope must be 'global' or 'project'");
        }

        let target = match tool {
            "cursor" => {
                let dir = self.scope_dir(tool, scope, root)?;
                dir.join(format!("{skill}.mdc"))
            }
            "codex" | "claude-code" => self.scope_dir(tool, scope, root)?.join(skill),
            "opencode" => {
                let dir = self.scope_dir(tool, scope, root)?;
                dir.join("AGENTS.md")
            }
            _ => bail!("unknown tool: {tool}"),
        };

        let existed = target.exists();
        if !existed {
            return Ok(InjectResult {
                tool: tool.to_string(),
                skill: skill.to_string(),
                scope: scope.to_string(),
                target: target.display().to_string(),
                status: "skipped".to_string(),
                message: format!("{skill} is not injected into {tool}"),
            });
        }

        match tool {
            "cursor" | "codex" | "claude-code" => {
                if target.is_dir() {
                    fs::remove_dir_all(&target).context("cannot remove skill folder")?;
                } else {
                    fs::remove_file(&target).context("cannot remove .mdc rule")?;
                }
            }
            "opencode" => remove_agents_section(&target, skill)?,
            _ => unreachable!(),
        }

        Ok(InjectResult {
            tool: tool.to_string(),
            skill: skill.to_string(),
            scope: scope.to_string(),
            target: target.display().to_string(),
            status: "injected".to_string(),
            message: format!("removed {skill} from {tool}"),
        })
    }

    fn scope_dir(&self, tool: &str, scope: &str, root: Option<&Path>) -> Result<PathBuf> {
        if scope == "global" {
            tools::global_dir(tool)
        } else {
            let root = root.ok_or_else(|| anyhow!("project scope requires a root path"))?;
            tools::project_dir(tool, root)
        }
    }
}

// ---------------------------------------------------------------------------
// Cursor .mdc
// ---------------------------------------------------------------------------

fn mdc_content(body: &str) -> String {
    format!("---\ndescription: PalHub-injected skill\n---\n\n{body}\n")
}

// ---------------------------------------------------------------------------
// OpenCode AGENTS.md
// ---------------------------------------------------------------------------

fn append_agents_section(path: &Path, name: &str, body: &str) -> Result<()> {
    let raw = if path.exists() {
        fs::read_to_string(path).context("cannot read AGENTS.md")?
    } else {
        String::new()
    };

    // Dedupe: remove existing PalHub-managed section for this skill.
    let mut cleaned = remove_section(&raw, name);
    if !cleaned.trim_end().is_empty() {
        cleaned = cleaned.trim_end().to_string() + "\n";
    }
    let section = format!(
        "{marker}{name} -->\n## {name}\n\n{body}\n{marker}{name} -->\n",
        marker = AGENTS_MARKER
    );
    cleaned.push_str(&section);
    fs::write(path, cleaned).context("cannot write AGENTS.md")
}

fn remove_section(raw: &str, name: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if !skipping && trimmed.starts_with(AGENTS_MARKER) {
            let marker_name = trimmed
                .trim_start_matches(AGENTS_MARKER)
                .trim_end_matches("-->")
                .trim();
            if marker_name == name {
                skipping = true;
                continue;
            }
        }
        if skipping && trimmed.starts_with(AGENTS_MARKER) {
            let marker_name = trimmed
                .trim_start_matches(AGENTS_MARKER)
                .trim_end_matches("-->")
                .trim();
            if marker_name == name {
                skipping = false;
                continue;
            }
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    // Tidy trailing blank lines.
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn remove_agents_section(path: &Path, name: &str) -> Result<()> {
    let raw = fs::read_to_string(path).context("cannot read AGENTS.md")?;
    let cleaned = remove_section(&raw, name);
    if cleaned.trim().is_empty() {
        fs::remove_file(path).context("cannot remove empty AGENTS.md")?;
    } else {
        fs::write(path, cleaned).context("cannot write AGENTS.md")?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).context("cannot create destination dir")?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).context("cannot copy file")?;
        }
    }
    Ok(())
}
