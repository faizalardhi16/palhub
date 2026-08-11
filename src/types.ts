// PalHub shared types (mirror of the Rust API spec in README §4)

export interface SkillMeta {
  name: string;
  description: string;
  version: string;
  tags: string[];
  license: string;
  source: string;
  path: string;
  size: number;
  installed_at: string;
}

export interface ProjectInfo {
  path: string;
  name: string;
  detected_tools: string[];
  has_package_json: boolean;
  has_git: boolean;
  has_agents_md: boolean;
  has_claude_md: boolean;
  injected: Record<string, string[]>;
}

export interface InjectResult {
  tool: string;
  skill: string;
  scope: "global" | "project";
  target: string;
  status: "injected" | "updated" | "skipped";
  message: string;
}

export interface AppInfo {
  version: string;
  store_dir: string;
  tools: Record<string, string | null>;
}

export interface TerminalSession {
  session_id: string;
  command: string;
  cwd: string;
  status: "running" | "exited" | "killed";
  exit_code: number | null;
}

export interface TerminalLine {
  session_id: string;
  stream: "stdout" | "stderr";
  line: string;
}

export type ToolId = "cursor" | "codex" | "claude-code" | "opencode";
export type Scope = "global" | "project";

export const TOOL_LABELS: Record<ToolId, string> = {
  cursor: "Cursor",
  codex: "Codex",
  "claude-code": "Claude Code",
  opencode: "OpenCode",
};
