import { useEffect, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { terminalKill, terminalList, terminalRun } from "../api";
import type { TerminalLine, TerminalSession } from "../types";

export default function TerminalView() {
  const [command, setCommand] = useState("");
  const [cwd, setCwd] = useState("");
  const [sessions, setSessions] = useState<TerminalSession[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [lines, setLines] = useState<TerminalLine[]>([]);
  const [error, setError] = useState<string | null>(null);
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    terminalList().then(setSessions).catch((e) => setError(String(e)));
    let un: UnlistenFn | undefined;
    listen<TerminalLine>("terminal://output", (ev) => {
      setLines((prev) => [...prev.slice(-2000), ev.payload]);
    }).then((fn) => (un = fn));
    return () => {
      un?.();
    };
  }, []);

  useEffect(() => {
    logRef.current?.scrollTo(0, logRef.current.scrollHeight);
  }, [lines]);

  const run = async () => {
    if (!command.trim()) return;
    setError(null);
    try {
      const id = await terminalRun(command.trim(), cwd.trim());
      setActive(id);
      setCommand("");
      setSessions(await terminalList());
    } catch (e) {
      setError(String(e));
    }
  };

  const kill = async (id: string) => {
    try {
      await terminalKill(id);
      setSessions(await terminalList());
    } catch (e) {
      setError(String(e));
    }
  };

  const activeLines = lines.filter((l) => l.session_id === active);

  return (
    <div className="view">
      <section className="panel">
        <h2>Run command</h2>
        <div className="install-row">
          <input
            className="text-input mono"
            placeholder="opencode run &quot;fix the bug in auth&quot;"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && run()}
          />
          <button className="btn primary" onClick={run} disabled={!command.trim()}>
            Run
          </button>
        </div>
        <div className="install-row" style={{ marginTop: 8 }}>
          <input
            className="text-input mono"
            placeholder="Working directory (leave empty = home)"
            value={cwd}
            onChange={(e) => setCwd(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && run()}
          />
        </div>
        {error && <div className="error">{error}</div>}
      </section>

      {sessions.length > 0 && (
        <section className="panel">
          <h2>Sessions</h2>
          <div className="session-list">
            {sessions.map((s) => (
              <div key={s.session_id} className={`session ${active === s.session_id ? "active" : ""}`} onClick={() => setActive(s.session_id)}>
                <span className="mono dim">{s.cwd}</span>
                <span className="mono session-cmd">{s.command}</span>
                <span className={`badge ${s.status === "running" ? "ok" : ""}`}>{s.status}</span>
                {s.status === "running" && (
                  <button className="btn small danger" onClick={(e) => { e.stopPropagation(); kill(s.session_id); }}>
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      <section className="panel">
        <h2>Live output {active ? `— ${active.slice(0, 8)}` : ""}</h2>
        <div className="term-log mono" ref={logRef}>
          {activeLines.length === 0 ? (
            <div className="dim">Run a command to see output…</div>
          ) : (
            activeLines.map((l, i) => (
              <div key={i} className={l.stream === "stderr" ? "term-err" : ""}>
                {l.line}
              </div>
            ))
          )}
        </div>
      </section>
    </div>
  );
}
