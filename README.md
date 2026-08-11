# PalHub

**Desktop skills hub for AI coding tools.** Install skills from npm/GitHub, then inject them into Cursor, Codex, Claude Code, or OpenCode — globally or per project.

> **Phase 1 scope:** A Tauri desktop app (Windows) that acts as a skills store + injector for 4 AI coding tools: **Cursor, Codex, Claude Code, OpenCode**. Skill sourcing from **npm** and **GitHub**. Injection into **global tool folders** or **project structure**.

---

## 1. What PalHub Does (Phase 1)

```
┌─────────────────────────────────────────────────────────┐
│  PalHub Desktop App (Tauri v2, React, Windows)          │
│                                                         │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────┐  │
│  │ Skills Store │   │ Project      │   │ Terminal   │  │
│  │ - list       │   │ - open root  │   │ - run cmd  │  │
│  │ - install    │   │ - detect     │   │ - live log │  │
│  │   npm/github │   │   tools      │   │            │  │
│  │ - remove     │   │ - inject     │   │            │  │
│  │ - refresh    │   │   skills     │   │            │  │
│  └──────┬───────┘   └──────┬───────┘   └──────┬─────┘  │
│         │                  │                  │        │
│         ▼                  ▼                  ▼        │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Rust Core (Tauri commands)                     │   │
│  │  store.rs · project.rs · inject.rs · terminal.rs│   │
│  └───────┬───────────────────────────────┬─────────┘   │
│          ▼                               ▼             │
│  ~/.palhub/skills/               Cursor/Codex/Claude/  │
│  (skill cache)                   OpenCode folders      │
└─────────────────────────────────────────────────────────┘
```

**User flow:**
1. Open PalHub → **Skills Store** tab → install a skill from npm or GitHub (e.g. `github:faizalardhi16/quivern`, `npm:@scope/skill-name`).
2. Open **Project** tab → point at a root folder (e.g. `D:\work\exon-api`). PalHub reads the folder and detects which coding tools are active there.
3. Click **Inject** — pick a skill + a tool (Cursor/Codex/Claude Code/OpenCode) + scope (global or this project). PalHub writes the skill into the tool's expected location.
4. Use the **Terminal** tab to run the tool inside the project (e.g. `opencode run "..."`), with live output streamed to the UI.

---

## 2. Skill Format

A skill is a **folder containing `SKILL.md`** (plus optional `references/`, `scripts/`, `templates/`, `assets/`).

```yaml
---
name: quivern
description: PRD & design discussion generator. Use for requirements, PRDs, design docs.
version: 1.0.0
tags: [prd, planning, design]
license: MIT
---
# body (markdown — the actual skill instructions)
```

Rules:
- `name` must be unique in the store (kebab-case).
- `description` is the one-liner used in the store UI and in injected rule files.
- Extra frontmatter fields are preserved as-is.

### Skill Store cache

Installed skills live in `~/.palhub/skills/<name>/`. The store folder is the **single source of truth**; injection *copies* (or references) from it into tool folders.

---

## 3. Injection Targets (Phase 1)

| Tool | Global location | Project location | Format |
|------|----------------|------------------|--------|
| **Cursor** | `~/.cursor/rules/<name>.mdc` | `<root>/.cursor/rules/<name>.mdc` | `.mdc` (YAML frontmatter + markdown) |
| **Codex** | `~/.codex/skills/<name>/` | `<root>/.codex/skills/<name>/` | `SKILL.md` directory |
| **Claude Code** | `~/.claude/skills/<name>/` | `<root>/.claude/skills/<name>/` | `SKILL.md` directory |
| **OpenCode** | `~/.config/opencode/AGENTS.md` | `<root>/AGENTS.md` | AGENTS.md section |

Details:

- **Cursor `.mdc`** — generated from SKILL.md frontmatter + body:
  ```markdown
  ---
  description: <skill description>
  globs:
  ---
  <skill body>
  ```
- **Codex / Claude Code `SKILL.md` dirs** — the whole skill folder (SKILL.md + references/scripts) is copied, so relative file references keep working.
- **OpenCode `AGENTS.md`** — a section is appended (or merged if the section already exists):
  ```markdown
  ## <name>
  <skill body>
  ```

`AGENTS.md` at the project root is shared by Codex, Claude Code, and OpenCode — PalHub **dedupes** injections so a skill injected into OpenCode at project scope also surfaces for Codex/Claude Code without duplicate entries.

---

## 4. Tauri Command API (invoke contract)

All commands are invoked from the React frontend via `@tauri-apps/api/core` `invoke()`.

### 4.1 Store

| Command | Signature | Description |
|---------|-----------|-------------|
| `store_list` | `() -> Vec<SkillMeta>` | List installed skills in `~/.palhub/skills/` |
| `store_install` | `(source: String, name: Option<String>) -> SkillMeta` | Install from npm or GitHub |
| `store_remove` | `(name: String)` | Remove a skill (also cleans injections referencing it — *Phase 1: removes from store only; injected copies are left for manual cleanup*) |
| `store_refresh` | `(name: String) -> SkillMeta` | Re-pull git / re-pack npm for an installed skill |

**`SkillMeta`:**
```ts
interface SkillMeta {
  name: string;
  description: string;
  version: string;
  tags: string[];
  license: string;
  source: string;      // e.g. "github:owner/repo" | "npm:@scope/pkg@1.2.3"
  path: string;        // absolute path in the store
  size: number;        // bytes
  installed_at: string; // ISO 8601
}
```

**Source syntax (`store_install`):**
| Source | Example | Behavior |
|--------|---------|----------|
| `github:owner/repo` | `github:faizalardhi16/quivern` | `git clone --depth 1`; expects `SKILL.md` at repo root |
| `github:owner/repo#subdir` | `github:acme/skills#skills/analyst` | clone, then use `subdir` as the skill folder |
| `npm:package` | `npm:@anthropic/skills` | `npm pack` + extract; find `SKILL.md` at tarball root or `skills/` subfolder |
| `npm:package@version` | `npm:pal-skills@0.3.1` | pinned version |

### 4.2 Project

| Command | Signature | Description |
|---------|-----------|-------------|
| `project_open` | `(path: String) -> ProjectInfo` | Read root folder; detect tools + injected skills |
| `project_inject` | `(tool: String, skill: String, scope: String, path: Option<String>) -> InjectResult` | Inject skill into a tool (global or project) |
| `project_uninject` | `(tool: String, skill: String, scope: String, path: Option<String>) -> InjectResult` | Remove injected skill |

**`ProjectInfo`:**
```ts
interface ProjectInfo {
  path: string;
  name: string;
  detected_tools: string[];       // ["cursor","codex","claude-code","opencode"]
  has_package_json: boolean;
  has_git: boolean;
  has_agents_md: boolean;
  has_claude_md: boolean;
  injected: Record<string, string[]>; // tool -> skill names found in this project
}
```

**`InjectResult`:**
```ts
interface InjectResult {
  tool: string;
  skill: string;
  scope: "global" | "project";
  target: string;     // absolute path written
  status: "injected" | "updated" | "skipped";
  message: string;
}
```

**Tool identifiers:** `cursor` | `codex` | `claude-code` | `opencode`
**Scope:** `global` | `project` (project requires `path`)

### 4.3 Terminal

| Command | Signature | Description |
|---------|-----------|-------------|
| `terminal_run` | `(command: String, cwd: String) -> String` | Run a command in `cwd`; returns a session id |
| `terminal_kill` | `(session_id: String)` | Kill a running session |
| `terminal_list` | `() -> Vec<TerminalSession>` | List active sessions |

Live output is streamed via Tauri events:
- `event: "terminal://output"` — `{ session_id, stream: "stdout"|"stderr", line: string }`
- `event: "terminal://exit"` — `{ session_id, code: number }`

### 4.4 App

| Command | Signature | Description |
|---------|-----------|-------------|
| `app_info` | `() -> AppInfo` | PalHub version, store dir, detected tool CLIs on the machine |
| `app_open_folder` | `(path: String)` | Open folder in OS file explorer |

**`AppInfo`:**
```ts
interface AppInfo {
  version: string;
  store_dir: string;
  tools: Record<string, string | null>; // tool -> executable path found in PATH (or null)
}
```

---

## 5. Project Structure

```
palhub/
├── README.md
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── src/                        # React frontend (dark theme)
│   ├── main.tsx
│   ├── App.tsx                 # tab shell: Store | Project | Terminal
│   ├── styles.css
│   ├── api.ts                  # typed invoke wrappers
│   ├── types.ts
│   └── views/
│       ├── StoreView.tsx       # skill cards, install form (npm/github), remove/refresh
│       ├── ProjectView.tsx     # folder open, tool detection, inject/uninject UI
│       └── TerminalView.tsx    # command input, cwd, live log
└── src-tauri/                  # Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── build.rs
    ├── icons/
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── commands/
        │   ├── mod.rs
        │   ├── store.rs
        │   ├── project.rs
        │   ├── terminal.rs
        │   └── app.rs
        └── core/
            ├── mod.rs
            ├── skill_store.rs  # store dir mgmt, SKILL.md parse, list/remove/refresh
            ├── sources.rs      # npm pack + git clone installers
            ├── tools.rs        # tool paths, detection, AGENTS.md dedupe
            └── injector.rs     # per-tool writers (cursor/codex/claude/opencode)
```

---

## 6. Developer Setup

### Prerequisites (Windows)

1. [Rust](https://rustup.rs) (stable)
2. [Node.js](https://nodejs.org) ≥ 20
3. Tauri v2 prerequisites: [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (preinstalled on Win 10/11), Visual Studio Build Tools (C++ workload) or MSVC

### Commands

```bash
npm install
npm run tauri dev        # run in dev mode
npm run tauri build      # build installer (NSIS/MSI) → src-tauri/target/release/bundle/
```

### Linux (for development/verification)

```bash
# system deps
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libsoup-3.0-dev javascriptcoregtk-4.1-dev
npm install
npm run tauri dev
```

### Verification

```bash
cargo check --manifest-path src-tauri/Cargo.toml   # Rust compiles
npm run build                                       # frontend builds
```

---

## 7. Roadmap (post-Phase 1)

- **Phase 2:** Expert skill packs (finance/analytics/sport/machine), cross-skill pipelines, Qoder CLI target, Cursor manual-open mode.
- **Phase 3:** Skill publishing (push store skill → GitHub), cost/usage tracking, MCP hub, `palhub` CLI companion.
