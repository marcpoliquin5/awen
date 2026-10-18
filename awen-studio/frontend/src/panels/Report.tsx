import { useState } from "react";
import { api, download } from "../api";
import { Card, Err } from "./common";

export function ReportPanel({ project }: { project: string }) {
  const [preview, setPreview] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const md = async () => {
    try {
      const text = await api<string>(`/api/projects/${project}/report.md`);
      setPreview(text);
      download(`${project}-report.md`, text, "text/markdown");
    } catch (e) { setError(String(e)); }
  };
  const json = async () => {
    try {
      const r = await api(`/api/projects/${project}/report`);
      download(`${project}-report.json`, JSON.stringify(r, null, 2), "application/json");
    } catch (e) { setError(String(e)); }
  };
  return (
    <Card title="Export report">
      <p className="hint">Bundles the project, its program and capability hashes, the last compile report, the full
        experiment history and package versions.</p>
      <div className="row">
        <button className="primary" onClick={md}>Markdown</button>
        <button onClick={json}>JSON</button>
      </div>
      <Err error={error} />
      {preview && <pre className="gen">{preview}</pre>}
    </Card>
  );
}
