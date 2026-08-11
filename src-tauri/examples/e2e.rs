//! End-to-end smoke test for PalHub core (not part of unit tests — requires network).
//! Run: cargo run --example e2e
//!
//! Flow: install a real skill from GitHub (anthropics/skills#skills/docx),
//! then inject it into a temp project for all 4 tools, verify output files,
//! then uninject and verify cleanup.

use std::fs;
use std::path::PathBuf;

use palhub_lib::core::injector::Injector;
use palhub_lib::core::skill_store::SkillStore;

fn main() {
    let base = std::env::temp_dir().join("palhub-e2e");
    let project = std::env::temp_dir().join("palhub-e2e-project");
    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&project);
    fs::create_dir_all(&project).unwrap();

    let store = SkillStore::at(base.clone()).unwrap();

    // 1. Install from GitHub with subdir → skill name "docx"
    println!("== install github:anthropics/skills#skills/docx ==");
    let meta = store
        .install("github:anthropics/skills#skills/docx", None)
        .expect("install failed");
    println!("   installed: {} v{} ({})", meta.name, meta.version, meta.source);
    assert_eq!(meta.name, "docx");
    assert!(meta.path.ends_with("docx"));
    assert!(fs::read_dir(&meta.path).unwrap().count() > 0);

    let list = store.list().unwrap();
    println!("   store list: {:?}", list.iter().map(|m| m.name.clone()).collect::<Vec<_>>());
    assert_eq!(list.len(), 1);

    // 2. Inject into the temp project for all 4 tools (project scope)
    let inj = Injector::new(&store);
    for tool in ["cursor", "codex", "claude-code", "opencode"] {
        let res = inj
            .inject(tool, "docx", "project", Some(&project))
            .unwrap_or_else(|e| panic!("inject {tool}: {e}"));
        println!("   inject {tool}: {} → {}", res.status, res.target);
        assert!(res.status == "injected" || res.status == "updated");
        assert!(PathBuf::from(&res.target).exists());
    }

    // 3. Verify each target file/folder
    assert!(project.join(".cursor/rules/docx.mdc").is_file());
    assert!(project.join(".codex/skills/docx/SKILL.md").is_file());
    assert!(project.join(".claude/skills/docx/SKILL.md").is_file());
    let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(agents.contains("<!-- palhub:docx -->"));
    assert!(agents.contains("## docx"));
    println!("   all 4 targets verified ✓");
    // 4. Re-inject opencode → dedupe (no duplicate section)
    inj.inject("opencode", "docx", "project", Some(&project))
        .unwrap();
    let agents2 = fs::read_to_string(project.join("AGENTS.md")).unwrap();
    // One section block contains exactly one "## docx" header (markers open+close = 2).
    let sections = agents2.matches("\n## docx\n").count();
    assert_eq!(sections, 1, "expected exactly 1 section, got {sections}");
    println!("   dedupe verified (1 section) ✓");

    // 5. Uninject all
    for tool in ["cursor", "codex", "claude-code", "opencode"] {
        let res = inj
            .uninject(tool, "docx", "project", Some(&project))
            .unwrap_or_else(|e| panic!("uninject {tool}: {e}"));
        println!("   uninject {tool}: {}", res.message);
    }
    assert!(!project.join(".cursor/rules/docx.mdc").exists());
    assert!(!project.join(".codex/skills/docx").exists());
    assert!(!project.join(".claude/skills/docx").exists());
    // AGENTS.md is removed entirely once empty (or no palhub section remains)
    let agents_clean = match fs::read_to_string(project.join("AGENTS.md")) {
        Ok(raw) => !raw.contains("palhub:docx"),
        Err(_) => true,
    };
    assert!(agents_clean);
    println!("   cleanup verified ✓");

    // 6. Remove from store
    store.remove("docx").unwrap();
    assert!(store.list().unwrap().is_empty());
    println!("   store cleaned ✓");

    let _ = fs::remove_dir_all(&base);
    let _ = fs::remove_dir_all(&project);
    println!("\nE2E PASSED ✅");
}
