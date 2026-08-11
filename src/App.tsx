import { useEffect, useState } from "react";
import StoreView from "./views/StoreView";
import ProjectView from "./views/ProjectView";
import TerminalView from "./views/TerminalView";
import { appInfo } from "./api";
import type { AppInfo } from "./types";

type Tab = "store" | "project" | "terminal";

export default function App() {
  const [tab, setTab] = useState<Tab>("store");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    appInfo()
      .then(setInfo)
      .catch((e) => console.error("app_info failed", e));
  }, []);

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "store", label: "Skills Store", icon: "📦" },
    { id: "project", label: "Project", icon: "📁" },
    { id: "terminal", label: "Terminal", icon: "⌨️" },
  ];

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-dot" />
          <h1>PalHub</h1>
          <span className="version">{info?.version ?? "…"}</span>
        </div>
        {info?.store_dir && (
          <div className="storedir" title={info.store_dir}>
            📂 {info.store_dir}
          </div>
        )}
      </header>

      <nav className="tabs">
        {tabs.map((t) => (
          <button
            key={t.id}
            className={`tab ${tab === t.id ? "active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            <span className="tab-icon">{t.icon}</span> {t.label}
          </button>
        ))}
      </nav>

      <main className="content">
        {tab === "store" && <StoreView />}
        {tab === "project" && <ProjectView />}
        {tab === "terminal" && <TerminalView />}
      </main>
    </div>
  );
}
