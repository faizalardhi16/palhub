import { useCallback, useEffect, useState } from "react";
import { storeInstall, storeList, storeRefresh, storeRemove } from "../api";
import type { SkillMeta } from "../types";

const SOURCE_EXAMPLES = [
  "github:faizalardhi16/quivern",
  "github:owner/repo#subdir",
  "npm:package-name",
  "npm:package-name@1.2.3",
];

export default function StoreView() {
  const [skills, setSkills] = useState<SkillMeta[]>([]);
  const [source, setSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const list = await storeList();
      setSkills(list);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const install = async () => {
    if (!source.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const meta = await storeInstall(source.trim());
      setNotice(`✅ Installed "${meta.name}" v${meta.version}`);
      setSource("");
      await refresh();
    } catch (e) {
      setError(`Install failed: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const remove = async (name: string) => {
    if (!confirm(`Remove skill "${name}" from the store?`)) return;
    setError(null);
    try {
      await storeRemove(name);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const refreshSkill = async (name: string) => {
    setError(null);
    try {
      await storeRefresh(name);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const filtered = skills.filter(
    (s) =>
      s.name.toLowerCase().includes(filter.toLowerCase()) ||
      s.tags.some((t) => t.toLowerCase().includes(filter.toLowerCase()))
  );

  return (
    <div className="view">
      <section className="panel">
        <h2>Install a skill</h2>
        <div className="install-row">
          <input
            className="text-input mono"
            placeholder="github:owner/repo  |  npm:package@version"
            value={source}
            onChange={(e) => setSource(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && install()}
            disabled={busy}
          />
          <button className="btn primary" onClick={install} disabled={busy || !source.trim()}>
            {busy ? "Installing…" : "Install"}
          </button>
        </div>
        <div className="hint">
          Examples:{" "}
          {SOURCE_EXAMPLES.map((ex) => (
            <button key={ex} className="chip" onClick={() => setSource(ex)}>
              {ex}
            </button>
          ))}
        </div>
        {error && <div className="error">{error}</div>}
        {notice && <div className="notice">{notice}</div>}
      </section>

      <section className="panel">
        <div className="panel-head">
          <h2>Installed skills ({filtered.length})</h2>
          <input
            className="text-input"
            placeholder="Filter by name / tag…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
        {filtered.length === 0 ? (
          <div className="empty">
            No skills yet. Install one from npm or GitHub above. 🚀
          </div>
        ) : (
          <div className="skill-grid">
            {filtered.map((s) => (
              <div key={s.name} className="skill-card">
                <div className="skill-head">
                  <span className="skill-name">{s.name}</span>
                  <span className="skill-version">v{s.version}</span>
                </div>
                <p className="skill-desc">{s.description || "—"}</p>
                <div className="skill-tags">
                  {s.tags.map((t) => (
                    <span key={t} className="tag">
                      {t}
                    </span>
                  ))}
                  {s.tags.length === 0 && <span className="tag dim">no tags</span>}
                </div>
                <div className="skill-foot">
                  <span className="skill-source mono" title={s.source}>
                    {s.source}
                  </span>
                  <div className="skill-actions">
                    <button className="btn small" onClick={() => refreshSkill(s.name)}>
                      ↻
                    </button>
                    <button className="btn small danger" onClick={() => remove(s.name)}>
                      ✕
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
