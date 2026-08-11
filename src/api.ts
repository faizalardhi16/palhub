import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  InjectResult,
  ProjectInfo,
  SkillMeta,
  TerminalSession,
} from "./types";

// ---- Store ----
export const storeList = () => invoke<SkillMeta[]>("store_list");
export const storeInstall = (source: string, name?: string) =>
  invoke<SkillMeta>("store_install", { source, name: name ?? null });
export const storeRemove = (name: string) => invoke<void>("store_remove", { name });
export const storeRefresh = (name: string) =>
  invoke<SkillMeta>("store_refresh", { name });

// ---- Project ----
export const projectOpen = (path: string) =>
  invoke<ProjectInfo>("project_open", { path });
export const projectInject = (
  tool: string,
  skill: string,
  scope: string,
  path?: string
) =>
  invoke<InjectResult>("project_inject", {
    tool,
    skill,
    scope,
    path: path ?? null,
  });
export const projectUninject = (
  tool: string,
  skill: string,
  scope: string,
  path?: string
) =>
  invoke<InjectResult>("project_uninject", {
    tool,
    skill,
    scope,
    path: path ?? null,
  });

// ---- Terminal ----
export const terminalRun = (command: string, cwd: string) =>
  invoke<string>("terminal_run", { command, cwd });
export const terminalKill = (sessionId: string) =>
  invoke<void>("terminal_kill", { sessionId });
export const terminalList = () =>
  invoke<TerminalSession[]>("terminal_list");

// ---- App ----
export const appInfo = () => invoke<AppInfo>("app_info");
export const appOpenFolder = (path: string) =>
  invoke<void>("app_open_folder", { path });
