import { useEffect, useState } from "react";
import { api, fmt } from "../api";
import { Card, Err, KV } from "./common";

export function CompilerPanel({ project }: { project: string }) {
  const [doc, setDoc] = useState<any>(null);
  const [report, setReport] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api(`/api/projects/${project}`).then((d) => { setDoc(d); setReport(d.compile_report); })
      .catch((e) => setError(String(e)));
  }, [project]);
  const compile = async () => {
    setError(null);
    try { setReport(await api(`/api/projects/${project}/compile`, {})); } catch (e) { setError(String(e)); }
  };
  return (
    <>
      <Card title="Program and target">
        {doc && <KV data={{ "ir version": doc.program.ir_version, ops: doc.program.ops.map((o: any) => `${o.op}:${o.id}`).join(", "),
          tensors: doc.program.tensors.map((t: any) => `${t.id}[${t.shape.join("×")}]`).join(", "),
          backend: doc.capabilities.backend_id }} />}
        <button className="primary" onClick={compile}>compile with the Rust compiler</button>
        <Err error={error} />
      </Card>
      {report && (report.ok ? (
        <>
          <Card title="Placement" tag="estimated">
            <table>
              <thead><tr><th>op</th><th>backend</th><th>latency ns</th><th>energy µJ</th><th>effective bits</th><th>error fraction</th></tr></thead>
              <tbody>{report.placement.map((p: any) => (
                <tr key={p.op_id}><td>{p.op_id}</td><td>{p.selected_backend}</td><td>{fmt(p.latency_ns)}</td>
                  <td>{fmt(p.energy_uj)}</td><td>{fmt(p.effective_bits)}</td><td>{fmt(p.estimated_error_fraction)}</td></tr>
              ))}</tbody>
            </table>
            <p className="hint">{report.labels.placement}</p>
          </Card>
          <Card title="Capability negotiation">
            <table>
              <thead><tr><th>backend</th><th>operation</th><th>eligible</th><th>diagnostics</th></tr></thead>
              <tbody>{report.capability_negotiations.map((c: any, i: number) => (
                <tr key={i}><td>{c.backend_id}</td><td>{c.operation}</td><td className={c.eligible ? "ok" : "bad"}>{String(c.eligible)}</td>
                  <td>{c.diagnostics.join("; ") || "—"}</td></tr>
              ))}</tbody>
            </table>
            <KV data={{ health: report.health?.status, "calibration profile": report.health?.calibration_profile_id,
              "compile seconds": fmt(report.compile_seconds) }} />
          </Card>
          <Card title="Diagnostics"><ul>{report.diagnostics.map((d: string, i: number) => <li key={i}>{d}</li>)}</ul></Card>
        </>
      ) : <Card title="Compile failed"><p className="bad">{report.error}</p></Card>)}
    </>
  );
}
