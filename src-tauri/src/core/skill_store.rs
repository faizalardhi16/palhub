use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::sources;

/// Metadata about an installed skill (mirrors the TS `SkillMeta` in README §4.1).
#[derive(Debug, Clone, Serialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub license: String,
    pub source: String,
    pub path: String,
    pub size: u64,
    pub installed_at: String,
    pub has_knowledge: bool,
    pub knowledge_files: u64,
}

/// Provenance record kept in `~/.palhub/registry.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub source: String,
    pub installed_at: String,
}

/// The skill store: `~/.palhub/` with `skills/<name>/` folders + `registry.json`.
pub struct SkillStore {
    pub base_dir: PathBuf,
}

impl SkillStore {
    pub fn new() -> Result<Self> {
        let base = dirs::home_dir()
            .ok_or_else(|| anyhow!("cannot resolve home directory"))?
            .join(".palhub");
        Self::at(base)
    }

    /// Create a store rooted at an explicit path (used by tests).
    pub fn at(base: PathBuf) -> Result<Self> {
        fs::create_dir_all(base.join("skills")).context("cannot create skills dir")?;
        Ok(Self { base_dir: base })
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.base_dir.join("skills")
    }

    fn registry_path(&self) -> PathBuf {
        self.base_dir.join("registry.json")
    }

    fn load_registry(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let path = self.registry_path();
        if !path.exists() {
            return Ok(serde_json::Map::new());
        }
        let raw = fs::read_to_string(&path).context("cannot read registry.json")?;
        serde_json::from_str(&raw).context("registry.json is corrupted")
    }

    fn save_registry(&self, reg: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
        let raw = serde_json::to_string_pretty(reg)?;
        fs::write(self.registry_path(), raw).context("cannot write registry.json")
    }

    fn set_registry_entry(&self, name: &str, source: &str) -> Result<()> {
        let mut reg = self.load_registry()?;
        reg.insert(
            name.to_string(),
            serde_json::json!({
                "source": source,
                "installed_at": Utc::now().to_rfc3339(),
            }),
        );
        self.save_registry(&reg)
    }

    fn remove_registry_entry(&self, name: &str) -> Result<()> {
        let mut reg = self.load_registry()?;
        reg.remove(name);
        self.save_registry(&reg)
    }

    pub fn list(&self) -> Result<Vec<SkillMeta>> {
        let reg = self.load_registry()?;
        let mut out = Vec::new();
        let skills_dir = self.skills_dir();
        let entries = fs::read_dir(&skills_dir)
            .with_context(|| format!("cannot read {}", skills_dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = self.read_skill(&name, &path, &reg).unwrap_or_else(|e| {
                // A broken skill folder should not crash the whole list.
                let (has_knowledge, knowledge_files) = knowledge_stats(&path);
                SkillMeta {
                    name: name.clone(),
                    description: format!("<unreadable: {e}>"),
                    version: String::new(),
                    tags: vec![],
                    license: String::new(),
                    source: reg
                        .get(&name)
                        .and_then(|v| v.get("source"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    path: path.display().to_string(),
                    size: 0,
                    installed_at: String::new(),
                    has_knowledge,
                    knowledge_files,
                }
            });
            out.push(meta);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn read_skill(
        &self,
        name: &str,
        dir: &Path,
        reg: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<SkillMeta> {
        let fm = parse_skill_md(dir)?;
        let size = dir_size(dir)?;
        let (has_knowledge, knowledge_files) = knowledge_stats(dir);
        let source = reg
            .get(name)
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str())
            .unwrap_or("local")
            .to_string();
        let installed_at = reg
            .get(name)
            .and_then(|v| v.get("installed_at"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(SkillMeta {
            name: fm.name.unwrap_or_else(|| name.to_string()),
            description: fm.description.unwrap_or_default(),
            version: fm.version.unwrap_or_else(|| "0.0.0".to_string()),
            tags: fm.tags,
            license: fm.license.unwrap_or_default(),
            source,
            path: dir.display().to_string(),
            size,
            installed_at,
            has_knowledge,
            knowledge_files,
        })
    }

    /// Install a skill from a source string (`github:...` or `npm:...`).
    pub fn install(&self, source: &str, name_override: Option<String>) -> Result<SkillMeta> {
        let source = source.trim();
        let (name, temp_dir) = sources::fetch(source)?;
        let name = name_override.unwrap_or(name);

        if name.is_empty() || name.contains(['/', '\\', ' ', '.']) {
            bail!("invalid skill name: {name:?}");
        }

        let dest = self.skills_dir().join(&name);
        if dest.exists() {
            fs::remove_dir_all(&dest).context("cannot replace existing skill folder")?;
        }
        // Move the fetched skill folder into the store.
        let src_dir = temp_dir.join(&name);
        if !src_dir.exists() {
            // Fallback: fetched content is at temp root and contains a SKILL.md.
            let root_has_skill = temp_dir.join("SKILL.md").exists();
            if root_has_skill {
                fs::rename(&temp_dir, &dest).context("cannot move skill into store")?;
            } else {
                bail!("no SKILL.md found in source");
            }
        } else {
            fs::rename(&src_dir, &dest).context("cannot move skill into store")?;
        }

        // Clean up temp dir (rename above consumed it; remove leftovers).
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(&temp_dir);
        }

        // Validate + record provenance.
        let fm = parse_skill_md(&dest).context("installed skill has invalid SKILL.md")?;
        self.set_registry_entry(&name, source)?;

        let size = dir_size(&dest)?;
        let (has_knowledge, knowledge_files) = knowledge_stats(&dest);
        let meta = SkillMeta {
            name: fm.name.unwrap_or(name),
            description: fm.description.unwrap_or_default(),
            version: fm.version.unwrap_or_else(|| "0.0.0".to_string()),
            tags: fm.tags,
            license: fm.license.unwrap_or_default(),
            source: source.to_string(),
            path: dest.display().to_string(),
            size,
            installed_at: Utc::now().to_rfc3339(),
            has_knowledge,
            knowledge_files,
        };
        Ok(meta)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let dest = self.skills_dir().join(name);
        if !dest.exists() {
            bail!("skill not found: {name}");
        }
        fs::remove_dir_all(&dest).context("cannot remove skill folder")?;
        self.remove_registry_entry(name)?;
        Ok(())
    }

    pub fn refresh(&self, name: &str) -> Result<SkillMeta> {
        let reg = self.load_registry()?;
        let entry = reg
            .get(name)
            .and_then(|v| v.get("source"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("skill not found in registry: {name}"))?;
        let source = entry.to_string();
        // Re-fetch into a temp dir, then swap.
        let (fetched_name, temp_dir) = sources::fetch(&source)?;
        let fetched = temp_dir.join(&fetched_name);
        let fetched = if fetched.exists() { fetched } else { temp_dir.clone() };
        let dest = self.skills_dir().join(name);
        let backup = self.base_dir.join(format!(".{name}.bak"));
        if backup.exists() {
            let _ = fs::remove_dir_all(&backup);
        }
        let had_dest = dest.exists();
        if had_dest {
            fs::rename(&dest, &backup).context("cannot move current skill aside")?;
        }
        match fs::rename(&fetched, &dest) {
            Ok(_) => {
                let _ = fs::remove_dir_all(&temp_dir);
                if backup.exists() {
                    let _ = fs::remove_dir_all(&backup);
                }
            }
            Err(e) => {
                // Roll back.
                if had_dest && backup.exists() {
                    let _ = fs::rename(&backup, &dest);
                }
                bail!("refresh failed: {e}");
            }
        }
        self.set_registry_entry(name, &source)?;
        let fm = parse_skill_md(&dest).context("refreshed skill has invalid SKILL.md")?;
        let (has_knowledge, knowledge_files) = knowledge_stats(&dest);
        Ok(SkillMeta {
            name: fm.name.unwrap_or_else(|| name.to_string()),
            description: fm.description.unwrap_or_default(),
            version: fm.version.unwrap_or_else(|| "0.0.0".to_string()),
            tags: fm.tags,
            license: fm.license.unwrap_or_default(),
            source,
            path: dest.display().to_string(),
            size: dir_size(&dest)?,
            installed_at: Utc::now().to_rfc3339(),
            has_knowledge,
            knowledge_files,
        })
    }

    /// Absolute path of an installed skill (validates existence).
    pub fn resolve(&self, name: &str) -> Result<PathBuf> {
        let p = self.skills_dir().join(name);
        if !p.exists() {
            bail!("skill not installed: {name}");
        }
        Ok(p)
    }
}

// ---------------------------------------------------------------------------
// SKILL.md frontmatter parsing (lightweight, no YAML dependency)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub license: Option<String>,
}

/// Parse `---\n...\n---` frontmatter from a SKILL.md. Returns defaults if absent.
pub fn parse_skill_md(dir: &Path) -> Result<Frontmatter> {
    let md_path = dir.join("SKILL.md");
    let raw = fs::read_to_string(&md_path)
        .with_context(|| format!("missing SKILL.md in {}", dir.display()))?;
    let body = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    if !body.trim_start().starts_with("---") {
        return Ok(Frontmatter::default());
    }
    let after = body.trim_start().trim_start_matches("---");
    let Some(end) = after.find("\n---") else {
        return Ok(Frontmatter::default());
    };
    let fm_raw = &after[..end];

    let mut fm = Frontmatter::default();
    let mut current_key: Option<String> = None;
    for line in fm_raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_lowercase();
            let v = v.trim();
            current_key = Some(k.clone());
            match k.as_str() {
                "name" => fm.name = Some(v.to_string()),
                "description" => fm.description = Some(v.to_string()),
                "version" => fm.version = Some(v.to_string()),
                "license" => fm.license = Some(v.to_string()),
                "tags" => {
                    let cleaned = v
                        .trim_matches(['[', ']'])
                        .split(',')
                        .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>();
                    if !cleaned.is_empty() {
                        fm.tags = cleaned;
                    }
                }
                _ => {}
            }
        } else if let Some(item) = line.strip_prefix('-') {
            // list item under the previous key
            if let Some(k) = current_key.as_deref() {
                let v = item.trim().trim_matches('"').trim_matches('\'').to_string();
                if !v.is_empty() {
                    match k {
                        "tags" => fm.tags.push(v),
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(fm)
}

/// Body of SKILL.md (everything after frontmatter) — used for injection.
pub fn skill_body(dir: &Path) -> Result<String> {
    let md_path = dir.join("SKILL.md");
    let raw = fs::read_to_string(&md_path)?;
    let body = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let trimmed = body.trim_start();
    if !trimmed.starts_with("---") {
        return Ok(body.to_string());
    }
    let after = trimmed.trim_start_matches("---");
    let Some(end) = after.find("\n---") else {
        return Ok(body.to_string());
    };
    let rest = &after[end + 4..];
    Ok(rest.trim_start().to_string())
}

pub fn dir_size(path: &Path) -> Result<u64> {
    fn walk(p: &Path, acc: &mut u64) -> Result<()> {
        for entry in fs::read_dir(p)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                walk(&entry.path(), acc)?;
            } else if ft.is_file() {
                *acc += entry.metadata()?.len();
            }
        }
        Ok(())
    }
    let mut acc = 0u64;
    walk(path, &mut acc)?;
    Ok(acc)
}

/// Detect a `knowledge/` bundle inside a skill folder: `(exists, file_count)`.
pub fn knowledge_stats(dir: &Path) -> (bool, u64) {
    let kdir = dir.join("knowledge");
    if !kdir.is_dir() {
        return (false, 0);
    }
    let mut count = 0u64;
    fn walk(p: &Path, count: &mut u64) {
        if let Ok(entries) = fs::read_dir(p) {
            for e in entries.flatten() {
                let ft = e.file_type();
                let Ok(ft) = ft else { continue };
                if ft.is_dir() {
                    walk(&e.path(), count);
                } else if ft.is_file() {
                    *count += 1;
                }
            }
        }
    }
    walk(&kdir, &mut count);
    (true, count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_skill(dir: &Path, frontmatter: &str, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("{frontmatter}\n{body}")).unwrap();
    }

    #[test]
    fn parses_frontmatter() {
        let dir = std::env::temp_dir().join("palhub-test-fm");
        let _ = fs::remove_dir_all(&dir);
        write_skill(
            &dir,
            "---\nname: quivern\ndescription: PRD generator\nversion: 1.2.0\ntags: [prd, planning]\nlicense: MIT\n---",
            "# Quivern\n\nBody here.",
        );
        let fm = parse_skill_md(&dir).unwrap();
        assert_eq!(fm.name.as_deref(), Some("quivern"));
        assert_eq!(fm.description.as_deref(), Some("PRD generator"));
        assert_eq!(fm.version.as_deref(), Some("1.2.0"));
        assert_eq!(fm.tags, vec!["prd", "planning"]);
        assert_eq!(fm.license.as_deref(), Some("MIT"));

        let body = skill_body(&dir).unwrap();
        assert!(body.contains("Body here."));
        assert!(!body.contains("---"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_list_style_tags() {
        let dir = std::env::temp_dir().join("palhub-test-tags");
        let _ = fs::remove_dir_all(&dir);
        write_skill(
            &dir,
            "---\nname: x\ntags:\n  - finance\n  - analyst\n---",
            "body",
        );
        let fm = parse_skill_md(&dir).unwrap();
        assert_eq!(fm.tags, vec!["finance", "analyst"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_install_local_source_rejects_unknown() {
        let base = std::env::temp_dir().join("palhub-test-store");
        let _ = fs::remove_dir_all(&base);
        let store = SkillStore::at(base.clone()).unwrap();
        // Only github:/npm: are supported; anything else must fail cleanly.
        let err = store.install("https://example.com/x", None).unwrap_err();
        assert!(err.to_string().contains("unsupported source"));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn knowledge_stats_detects_bundle() {
        let dir = std::env::temp_dir().join("palhub-test-knowledge");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("knowledge").join("topik")).unwrap();
        fs::write(dir.join("SKILL.md"), "# Skill").unwrap();
        fs::write(dir.join("knowledge").join("index.md"), "# Index").unwrap();
        fs::write(dir.join("knowledge").join("topik").join("a.md"), "a").unwrap();
        fs::write(dir.join("knowledge").join("topik").join("b.md"), "b").unwrap();

        let (has, count) = knowledge_stats(&dir);
        assert!(has);
        assert_eq!(count, 3);

        // No knowledge dir → (false, 0).
        let plain = std::env::temp_dir().join("palhub-test-plain");
        let _ = fs::remove_dir_all(&plain);
        fs::create_dir_all(&plain).unwrap();
        fs::write(plain.join("SKILL.md"), "# S").unwrap();
        let (has, count) = knowledge_stats(&plain);
        assert!(!has);
        assert_eq!(count, 0);

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&plain);
    }

    #[test]
    fn local_source_install_copies_knowledge() {
        let base = std::env::temp_dir().join("palhub-test-local");
        let _ = fs::remove_dir_all(&base);
        let store = SkillStore::at(base.clone()).unwrap();

        // Build a local skill folder with a knowledge bundle.
        let local = std::env::temp_dir().join("palhub-test-localsrc").join("finance-id");
        let _ = fs::remove_dir_all(&local);
        fs::create_dir_all(local.join("knowledge")).unwrap();
        fs::write(
            local.join("SKILL.md"),
            "---\nname: finance-id\ndescription: Finance knowledge\n---\n# Body",
        )
        .unwrap();
        fs::write(local.join("knowledge").join("index.md"), "# Index").unwrap();

        let meta = store
            .install(&format!("local:{}", local.display()), None)
            .unwrap();
        assert_eq!(meta.name, "finance-id");
        assert!(meta.has_knowledge);
        assert_eq!(meta.knowledge_files, 1);

        // Installed folder carries knowledge/.
        let dest = store.resolve("finance-id").unwrap();
        assert!(dest.join("knowledge").join("index.md").exists());

        let _ = fs::remove_dir_all(&base);
        let _ = fs::remove_dir_all(&local);
    }
}
