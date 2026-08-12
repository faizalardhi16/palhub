use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

use super::skill_store::{knowledge_stats, skill_body, SkillStore};
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
        let (has_knowledge, _) = knowledge_stats(&skill_dir);

        let (target_dir, target_file, existed) = match tool {
            "cursor" => {
                let dir = self.scope_dir(tool, scope, root)?;
                let file = dir.join(format!("{skill}.mdc"));
                let existed = file.exists();
                (dir, file, existed)
            }
            "codex" | "claude-code" => {
                let dir = self.scope_dir(tool, scope, root)?.join(skill);
                let existed = dir.exists();
                let _ = fs::remove_dir_all(&dir);
                copy_dir(&skill_dir, &dir)?;
                (dir.clone(), dir, existed)
            }
            "opencode" => {
                let dir = self.scope_dir(tool, scope, root)?;
                let file = dir.join("AGENTS.md");
                let existed = file.exists();
                (dir, file, existed)
            }
            _ => bail!("unknown tool: {tool}"),
        };

        fs::create_dir_all(&target_dir)
            .with_context(|| format!("cannot create {}", target_dir.display()))?;

        match tool {
            "cursor" => {
                let body = with_knowledge_note(&body, &skill_dir, &target_dir, skill, has_knowledge);
                fs::write(&target_file, mdc_content(&body)).context("cannot write .mdc rule")?
            }
            "opencode" => {
                let body = with_knowledge_note(&body, &skill_dir, &target_dir, skill, has_knowledge);
                append_agents_section(&target_file, skill, &body)?
            }
            _ => {}
        }
        // Copy the knowledge bundle next to the injected rule for tools that
        // only write a single file (Cursor .mdc / OpenCode AGENTS.md). Folder-
        // copy tools (codex/claude-code) already carry knowledge/ via copy_dir.
        if has_knowledge && matches!(tool, "cursor" | "opencode") {
            copy_knowledge(&skill_dir, &target_dir, skill)?;
        }
        let status = if existed { "updated" } else { "injected" };
        let kwnote = if has_knowledge { " (knowledge bundle included)" } else { "" };
        Ok(InjectResult {
            tool: tool.to_string(),
            skill: skill.to_string(),
            scope: scope.to_string(),
            target: target_file.display().to_string(),
            status: status.to_string(),
            message: format!(
                "{} → {}{}{}",
                skill,
                target_file.display(),
                if existed { " (updated)" } else { " (injected)" },
                kwnote
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
        // Clean up the copied knowledge bundle (single-file tools only).
        if matches!(tool, "cursor" | "opencode") {
            let scope_dir = self.scope_dir(tool, scope, root)?;
            let knowledge_dir = scope_dir.join(knowledge_dir_name(skill));
            if knowledge_dir.is_dir() {
                fs::remove_dir_all(&knowledge_dir).context("cannot remove knowledge bundle")?;
            }
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
// Knowledge bundle helpers
// ---------------------------------------------------------------------------

/// Knowledge folder name as copied next to a single-file rule.
fn knowledge_dir_name(skill: &str) -> String {
    format!(".{skill}.knowledge")
}

/// If the skill carries a `knowledge/` bundle, append a pointer to the injected
/// body so the model knows where to look (relative to the rule file).
fn with_knowledge_note(
    body: &str,
    skill_dir: &Path,
    target_dir: &Path,
    skill: &str,
    has_knowledge: bool,
) -> String {
    if !has_knowledge {
        return body.to_string();
    }
    let rel = knowledge_dir_name(skill);
    // Prefer a relative path from the rule file to the knowledge folder; fall
    // back to the absolute store path if target_dir is empty (unexpected).
    let loc = if target_dir.as_os_str().is_empty() {
        skill_dir.join("knowledge").display().to_string()
    } else {
        format!("./{rel}/")
    };
    format!(
        "{body}\n\n## Knowledge\n\nSkill ini membawa domain knowledge bundle (di-refresh harian).\nBaca file di dalam folder `{loc}` (mulai dari `index.md`) sebelum menjawab\npertanyaan yang butuh fakta domain. Setiap catatan punya frontmatter dengan\nsumber resmi (`url`, `source`, `tier`, `date`) — selalu cek tanggalnya.\n"
    )
}

/// Copy `<skill>/knowledge/` → `<target_dir>/.<skill>.knowledge/`.
fn copy_knowledge(skill_dir: &Path, target_dir: &Path, skill: &str) -> Result<()> {
    let src = skill_dir.join("knowledge");
    if !src.is_dir() {
        return Ok(());
    }
    let dst = target_dir.join(knowledge_dir_name(skill));
    let _ = fs::remove_dir_all(&dst);
    copy_dir(&src, &dst)?;
    Ok(())
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Build a temp skill folder with SKILL.md + a knowledge/ bundle.
    fn make_skill(parent: &Path, name: &str) -> PathBuf {
        let dir = parent.join(name);
        fs::create_dir_all(dir.join("knowledge").join("topik")).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: finance-id\ndescription: Finance knowledge\n---\n# Body",
        )
        .unwrap();
        fs::write(dir.join("knowledge").join("index.md"), "# Index").unwrap();
        fs::write(dir.join("knowledge").join("topik").join("pph21.md"), "# PPh 21").unwrap();
        dir
    }

    #[test]
    fn inject_knowledge_bundle_to_all_tools() {
        let base = std::env::temp_dir().join("palhub-test-injector");
        let _ = fs::remove_dir_all(&base);
        let store = SkillStore::at(base.join("store")).unwrap();
        let local = make_skill(&base, "finance-id");
        let meta = store.install(&format!("local:{}", local.display()), None).unwrap();
        assert!(meta.has_knowledge);
        assert_eq!(meta.knowledge_files, 2);

        let injector = Injector::new(&store);
        let project = base.join("project");
        fs::create_dir_all(&project).unwrap();

        for tool in ["cursor", "codex", "claude-code", "opencode"] {
            let res = injector.inject(tool, "finance-id", "project", Some(&project)).unwrap();
            assert!(res.status == "injected", "{tool}: {0}", res.message);
            // Folder-copy tools carry knowledge/ directly; single-file tools get
            // a copied `.finance-id.knowledge/` bundle.
            if tool == "codex" || tool == "claude-code" {
                let skill_dir = tools::project_dir(tool, &project).unwrap().join("finance-id");
                assert!(skill_dir.join("knowledge").join("index.md").exists(), "{tool}");
            } else {
                let scope = tools::project_dir(tool, &project).unwrap();
                let kdir = scope.join(".finance-id.knowledge");
                assert!(kdir.join("index.md").exists(), "{tool}: {0}", res.message);
                // Body pointer present.
                let raw = fs::read_to_string(&res.target).unwrap();
                assert!(raw.contains("## Knowledge"), "{tool}");
            }
        }

        // Uninject must remove the knowledge bundle too.
        for tool in ["cursor", "codex", "claude-code", "opencode"] {
            injector.uninject(tool, "finance-id", "project", Some(&project)).unwrap();
        }
        let scope = tools::project_dir("cursor", &project).unwrap();
        assert!(!scope.join(".finance-id.knowledge").exists());
        assert!(!scope.join("finance-id.mdc").exists());
        assert!(!tools::project_dir("codex", &project).unwrap().join("finance-id").exists());
        assert!(!tools::project_dir("opencode", &project).unwrap().join("AGENTS.md").exists());

        let _ = fs::remove_dir_all(&base);
    }
}
