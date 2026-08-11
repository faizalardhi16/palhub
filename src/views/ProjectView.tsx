import { useCallback, useEffect, useState } from "react";
import { appOpenFolder, projectInject, projectOpen, projectUninject, storeList } from "../api";
import type { ProjectInfo, Scope, SkillMeta, ToolId } from "../types";
import { TOOL_LABELS } from "../types";

const TOOLS: ToolId[] = ["cursor", "codex", "claude-code", "opencode"];

export default function ProjectView() {
  const [path, setPath] = useState("");
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // injection form state
  const [selTool, setSelTool] = useState<ToolId>("opencode");
  const [selSkill, setSelSkill] = useState("");
  const [selScope, setSelScope] = useState<Scope>("project");

  useEffect(() => {
    storeList()
      .then(setSkills)
      .catch((e) => setError(String(e)));
  }, []);

  const open = useCallback(
    async (p?: string) => {
      const target = (p ?? path).trim();
      if (!target) return;
      setBusy(true);
      setError(null);
      setNotice(null);
      try {
        const info = await projectOpen(target);
        setProject(info);
        setPath(info.path);
      } catch (e) {
        setError(`Cannot open folder: ${e}`);
      } finally {
        setBusy(false);
      }
    },
    [path]
  );

  const inject = async () => {
    if (!project || !selSkill) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const res = await projectInject(
        selTool,
        selSkill,
        selScope,
        selScope === "project" ? project.path : undefined
      );
      setNotice(`${res.status.toUpperCase()}: ${res.message}`);
      if (selScope === "project") await open(project.path);
    } catch (e) {
      setError(`Inject failed: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const uninject = async (tool: ToolId, skill: string) => {
    if (!project) return;
    setError(null);
    try {
      await projectUninject(tool, skill, "project", project.path);
      await open(project.path);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="view">
      <section className="panel">
        <h2>Open project</h2>
        <div className="install-row">
          <input
            className="text-input mono"
            placeholder="D:\\work\\my-project   (or /home/user/project)"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && open()}
            disabled={busy}
          />
          <button className="btn primary" onClick={() => open()} disabled={busy || !path.trim()}>
            {busy ? "Reading…" : "Open"}
          </button>
          <button
            className="btn"
            onClick={() => appOpenFolder(path).catch((e) => setError(String(e)))}
            disabled={!path.trim()}
            title="Open in file explorer"
          >
            📂
          </button>
        </div>
        {error && <div className="error">{error}</div>}
        {notice && <div className="notice">{notice}</div>}
      </section>

      {project && (
        <>
          <section className="panel">
            <div className="panel-head">
              <h2>📁 {project.name}</h2>
              <span className="mono dim">{project.path}</span>
            </div>
            <div className="badge-row">
              <span className={`badge ${project.detected_tools.length ? "ok" : ""}`}>
                tools: {project.detected_tools.length ? project.detected_tools.join(", ") : "none detected"}
              </span>
              <span className={`badge ${project.has_agents_md ? "ok" : ""}`}>AGENTS.md {project.has_agents_md ? "✓" : "✗"}</span>
              <span className={`badge ${project.has_claude_md ? "ok" : ""}`}>CLAUDE.md {project.has_claude_md ? "✓" : "✗"}</span>
              <span className={`badge ${project.has_package_json ? "ok" : ""}`}>package.json {project.has_package_json ? "✓" : "✗"}</span>
              <span className={`badge ${project.has_git ? "ok" : ""}`}>git {project.has_git ? "✓" : "✗"}</span>
            </div>
          </section>

          <section className="panel">
            <h2>Inject skill</h2>
            <div className="inject-row">
              <select
                className="select"
                value={selTool}
                onChange={(e) => setSelTool(e.target.value as ToolId)}
              >
                {TOOLS.map((t) => (
                  <option key={t} value={t}>
                    {TOOL_LABELS[t]}
                  </option>
                ))}
              </select>
              <select
                className="select"
                value={selSkill}
                onChange={(e) => setSelSkill(e.target.value)}
              >
                <option value="">— pick skill —</option>
                {skills.map((s) => (
                  <option key={s.name} value={s.name}>
                    {s.name}
                  </option>
                ))}
              </select>
              <select
                className="select"
                value={selScope}
                onChange={(e) => setSelScope(e.target.value as Scope)}
              >
                <option value="project">This project</option>
                <option value="global">Global</option>
              </select>
              <button
                className="btn primary"
                onClick={inject}
                disabled={busy || !selSkill}
              >
                Inject
              </button>
            </div>
            {skills.length === 0 && (
              <div className="hint">No skills in store — install some first (Skills Store tab).</div>
            )}
          </section>

          <section className="panel">
            <h2>Injected in this project</h2>
            {Object.entries(project.injected).length === 0 ? (
              <div className="empty">Nothing injected yet.</div>
            ) : (
              <div className="injected-list">
                {TOOLS.filter((t) => (project.injected[t] ?? []).length > 0).map((t) => (
                  <div key={t} className="injected-tool">
                    <span className="tool-name">{TOOL_LABELS[t]}</span>
                    <div className="injected-skills">
                      {(project.injected[t] ?? []).map((skill) => (
                        <span key={skill} className="chip">
                          {skill}
                          <button className="chip-x" onClick={() => uninject(t, skill)} title={`Remove ${skill} from ${TOOL_LABELS[t]}`}>
                            ✕
                          </button>
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </>
      )}
    </div>
  );
}
