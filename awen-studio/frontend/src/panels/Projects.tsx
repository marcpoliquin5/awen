import { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Err } from "./common";

export function ProjectsPanel({ selected, onSelect }: { selected: string | null; onSelect: (n: string) => void }) {
  const [data, setData] = useState<any>(null);
  const [name, setName] = useState("my-first-project");
  const [example, setExample] = useState("gemm-4x4");
  const [error, setError] = useState<string | null>(null);
  const load = () => api("/api/projects").then(setData).catch((e) => setError(String(e)));
  useEffect(() => { load(); }, []);

  const create = async () => {
    setError(null);
    try {
      await api("/api/projects", { name, example });
      onSelect(name);
      load();
    } catch (e) { setError(String(e)); }
  };

  return (
    <>
      <Card title="Projects">
        {data?.projects?.length ? (
          <table>
            <thead><tr><th>name</th><th>example</th><th>created (UTC)</th><th>experiments</th><th>compiled</th><th /></tr></thead>
            <tbody>
              {data.projects.map((p: any) => (
                <tr key={p.name} className={p.name === selected ? "sel" : ""}>
                  <td>{p.name}</td><td>{p.example}</td><td>{p.created_at}</td><td>{p.experiments}</td>
                  <td>{p.has_compile_report ? "yes" : "no"}</td>
                  <td><button onClick={() => onSelect(p.name)}>open</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : <p>No projects yet.</p>}
      </Card>
      <Card title="New project">
        <div className="row">
          <label>name <input value={name} onChange={(e) => setName(e.target.value)} /></label>
          <label>example
            <select value={example} onChange={(e) => setExample(e.target.value)}>
              {data?.examples?.map((x: any) => (
                <option key={x.id} value={x.id} disabled={!x.available}>{x.id}</option>
              ))}
            </select>
          </label>
          <button className="primary" onClick={create}>create</button>
        </div>
        <p className="hint">A project holds a tensor program, a device-capability document, its compile report and a
          Lab Agent store with its experiment history.</p>
        <Err error={error} />
      </Card>
    </>
  );
}
