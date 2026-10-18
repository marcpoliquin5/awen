import { useEffect, useState } from "react";
import { api } from "./api";
import { ProjectsPanel } from "./panels/Projects";
import { ModelPanel } from "./panels/Model";
import { CompilerPanel } from "./panels/Compiler";
import { OpsPanel } from "./panels/Ops";
import { NoisePanel } from "./panels/Noise";
import { ComparePanel } from "./panels/Compare";
import { ExperimentsPanel } from "./panels/Experiments";
import { ReportPanel } from "./panels/Report";

const TABS = ["Projects", "Model", "Compiler", "Ops", "Noise", "Compare", "Experiments", "Report"] as const;
type Tab = (typeof TABS)[number];

// The URL hash keeps the view shareable and reload-safe: #<Tab>[/<project>].
function fromHash(): [Tab, string | null] {
  const [t, p] = decodeURIComponent(window.location.hash.slice(1)).split("/");
  return [(TABS as readonly string[]).includes(t) ? (t as Tab) : "Projects", p || null];
}

export default function App() {
  const [tab, setTab] = useState<Tab>(() => fromHash()[0]);
  const [project, setProject] = useState<string | null>(() => fromHash()[1]);
  useEffect(() => {
    window.location.hash = project ? `${tab}/${project}` : tab;
  }, [tab, project]);
  const [health, setHealth] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api("/api/health").then(setHealth).catch((e) => setError(String(e)));
  }, []);

  const needsProject = ["Compiler", "Experiments", "Report"].includes(tab) && !project;
  return (
    <div className="shell">
      <header>
        <div className="brand">
          <span className="logo" aria-hidden="true" />
          <div>
            <h1>AWEN Studio</h1>
            <p className="sub">local · simulation only · no photonic hardware attached</p>
          </div>
        </div>
        <div className="status">
          {error ? <span className="bad">backend unreachable</span> : health ? (
            <span className="ok">backend ok · awen-native {health.versions["awen-native"] ?? "?"}</span>
          ) : "connecting…"}
          {project && <span className="pill">project: {project}</span>}
        </div>
      </header>
      <nav>
        {TABS.map((t) => (
          <button key={t} className={t === tab ? "active" : ""} onClick={() => setTab(t)}>{t}</button>
        ))}
      </nav>
      <main>
        {needsProject ? (
          <section className="card"><p>Select or create a project first (Projects tab).</p></section>
        ) : (
          <>
            {tab === "Projects" && <ProjectsPanel selected={project} onSelect={setProject} />}
            {tab === "Model" && <ModelPanel />}
            {tab === "Compiler" && project && <CompilerPanel project={project} />}
            {tab === "Ops" && <OpsPanel />}
            {tab === "Noise" && <NoisePanel />}
            {tab === "Compare" && <ComparePanel />}
            {tab === "Experiments" && project && <ExperimentsPanel project={project} />}
            {tab === "Report" && project && <ReportPanel project={project} />}
          </>
        )}
      </main>
      <footer>
        Every photonic number is <b>simulated</b> by AWEN's software models; compiler costs are <b>estimates</b>;
        CPU timings are <b>measured</b> on this machine.
      </footer>
    </div>
  );
}
