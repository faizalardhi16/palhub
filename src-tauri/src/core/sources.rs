use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};

/// Fetch a skill from `github:owner/repo[#subdir]` or `npm:package[@version]`.
///
/// Returns `(skill_name, temp_dir)` where the skill content lives at
/// `temp_dir/<skill_name>/SKILL.md` (or `temp_dir/SKILL.md` as a fallback).
pub fn fetch(source: &str) -> Result<(String, PathBuf)> {
    let source = source.trim();
    if let Some(rest) = source.strip_prefix("github:") {
        fetch_github(rest)
    } else if let Some(rest) = source.strip_prefix("npm:") {
        fetch_npm(rest)
    } else {
        bail!("unsupported source (use github:owner/repo or npm:package): {source}")
    }
}

fn temp_dir(tag: &str) -> Result<PathBuf> {
    let base = std::env::temp_dir().join("palhub");
    fs::create_dir_all(&base).context("cannot create temp dir")?;
    let dir = base.join(format!("{}-{}", tag, std::process::id()));
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
    }
    fs::create_dir_all(&dir).context("cannot create fetch temp dir")?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// GitHub
// ---------------------------------------------------------------------------

fn fetch_github(spec: &str) -> Result<(String, PathBuf)> {
    // spec: owner/repo or owner/repo#subdir
    let (repo, subdir) = match spec.split_once('#') {
        Some((r, s)) => (r, Some(s.to_string())),
        None => (spec, None),
    };
    if !repo.contains('/') {
        bail!("github source must be owner/repo: {spec}");
    }
    let (owner, repo_name) = repo
        .split_once('/')
        .ok_or_else(|| anyhow!("bad github source"))?;
    let repo_name = repo_name.trim_end_matches(".git");
    // When a subdir is given, the skill takes its name from the subdir's last segment.
    let skill_name = match &subdir {
        Some(s) => s
            .trim_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(repo_name)
            .to_string(),
        None => repo_name.to_string(),
    };

    let dir = temp_dir("github")?;
    let url = format!("https://github.com/{owner}/{repo_name}.git");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", &url, &repo_name])
        .current_dir(&dir)
        .status()
        .context("git not found — is Git installed?")?;
    if !status.success() {
        bail!("git clone failed for {url}");
    }

    let cloned = dir.join(&repo_name);
    let skill_root = match &subdir {
        Some(s) => cloned.join(s.trim_matches('/')),
        None => cloned.clone(),
    };
    if !skill_root.join("SKILL.md").exists() {
        bail!(
            "no SKILL.md found in {:?} — skill repos must contain SKILL.md at the root (or use #subdir)",
            skill_root
        );
    }

    // If a subdir was requested, stage it as the skill folder (replace the clone).
    if let Some(s) = &subdir {
        let s = s.trim_matches('/');
        let src = cloned.join(s);
        if !src.exists() {
            bail!("subdir not found: {s}");
        }
        let staged = dir.join(&skill_name);
        if staged.exists() {
            let _ = fs::remove_dir_all(&staged);
        }
        fs::rename(&src, &staged).context("cannot stage subdir skill")?;
        let _ = fs::remove_dir_all(&cloned);
    }

    Ok((skill_name, dir))
}

// ---------------------------------------------------------------------------
// npm
// ---------------------------------------------------------------------------

fn fetch_npm(spec: &str) -> Result<(String, PathBuf)> {
    // spec: package or package@version
    let (pkg, version) = match spec.split_once('@') {
        Some((p, v)) if !p.is_empty() => (p, Some(v.to_string())),
        _ => (spec, None),
    };
    // npm scoped packages: @scope/name — handle @scope/name@version correctly.
    // If spec starts with '@', the first '@' is the scope marker.
    let (pkg, version) = if spec.starts_with('@') {
        match spec.rfind('@') {
            Some(idx) if idx > 1 => (&spec[..idx], Some(spec[idx + 1..].to_string())),
            _ => (spec, None),
        }
    } else {
        (pkg, version)
    };

    let skill_name = pkg
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow!("bad npm package name"))?
        .to_string();

    let dir = temp_dir("npm")?;
    let full = match &version {
        Some(v) => format!("{pkg}@{v}"),
        None => pkg.to_string(),
    };

    let status = Command::new("npm")
        .args(["pack", &full, "--pack-destination", dir.to_str().unwrap_or_default()])
        .status()
        .context("npm not found — is Node.js/npm installed?")?;
    if !status.success() {
        bail!("npm pack failed for {full}");
    }

    // Find the .tgz produced by npm pack.
    let tgz = fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == "tgz").unwrap_or(false))
        .ok_or_else(|| anyhow!("npm pack produced no tarball"))?;

    // Extract tarball into <dir>/extract.
    let extract = dir.join("extract");
    fs::create_dir_all(&extract).context("cannot create extract dir")?;
    extract_tar_gz(&tgz, &extract)?;

    // Find the skill folder: SKILL.md at tarball root (package/) or skills/ subfolder.
    let root_candidates = fs::read_dir(&extract)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();

    let skill_root = root_candidates
        .iter()
        .find(|p| p.join("SKILL.md").exists())
        .cloned()
        .or_else(|| {
            // Look for <pkg>/skills/<something>/SKILL.md or <pkg>/skill/SKILL.md
            root_candidates.iter().find_map(|p| {
                let skills_dir = p.join("skills");
                if skills_dir.is_dir() {
                    fs::read_dir(&skills_dir).ok().and_then(|it| {
                        it.flatten()
                            .map(|e| e.path())
                            .find(|q| q.join("SKILL.md").exists())
                    })
                } else {
                    let skill_dir = p.join("skill");
                    if skill_dir.join("SKILL.md").exists() {
                        Some(skill_dir)
                    } else {
                        None
                    }
                }
            })
        })
        .ok_or_else(|| anyhow!("npm package contains no SKILL.md"))?;

    // Stage the skill folder as <dir>/<skill_name> for the store.
    let staged = dir.join(&skill_name);
    if staged.exists() {
        let _ = fs::remove_dir_all(&staged);
    }
    fs::rename(&skill_root, &staged).context("cannot stage npm skill")?;

    Ok((skill_name, dir))
}

// ---------------------------------------------------------------------------
// tar.gz extraction (flate2 + tar)
// ---------------------------------------------------------------------------

fn extract_tar_gz(tgz: &PathBuf, dest: &PathBuf) -> Result<()> {
    let file = fs::File::open(tgz).context("cannot open tarball")?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest).context("cannot extract tarball")?;
    Ok(())
}
