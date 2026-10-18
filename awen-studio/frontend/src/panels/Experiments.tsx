import { useEffect, useState } from "react";
import { api, fmt } from "../api";
import { Card, Err } from "./common";

const DEFAULT = JSON.stringify({ kind: "photonic_gemm", target: "photonic-sim", shape_mkn: [32, 128, 16],
  contract: { photons_per_mac: 1000 }, seeds: [1, 2, 3] }, null, 2);

export function ExperimentsPanel({ project }: { project: string }) {
  const [data, setData] = useState<any>(null);
  const [spec, setSpec] = useState(DEFAULT);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = () => api(`/api/projects/${project}/experiments`).then(setData).catch((e) => setError(String(e)));
  useEffect(() => { load(); }, [project]);
  const run = async (workflow: string, ref?: string) => {
    setBusy(true); setError(null);
    try {
      const body = workflow === "repro" ? { workflow, ref } : { workflow, experiment: JSON.parse(spec) };
      await api(`/api/projects/${project}/experiments`, body);
      await load();
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const summary = (h: any) => {
    if (h.metrics.checks) return h.metrics.checks.map((c: any) => `${c.passed ? "✓" : "✗"} ${c.check}`).join(" · ");
    if ("reproduced" in h.metrics) return `reproduced: ${h.metrics.reproduced}`;
    if (h.metrics.runs) return h.metrics.runs.map((r: any) => `seed ${r.seed}: rel RMS ${fmt(r.relative_rms_error ?? r.total_variation_vs_exact)}`).join(" · ");
    return "";
  };
  return (
    <>
      <Card title="Run an experiment (Lab Agent, simulation only)" tag="simulated">
        <textarea rows={8} value={spec} onChange={(e) => setSpec(e.target.value)} spellCheck={false} />
        <div className="row">
          <button className="primary" disabled={busy} onClick={() => run("quick")}>quick</button>
          <button disabled={busy} onClick={() => run("truth")}>truth checks</button>
        </div>
        <p className="hint">Targets: photonic-sim, awen-reference, qphotonic-sim. Any hardware target is refused.</p>
        <Err error={error} />
      </Card>
      <Card title="History">
        {data && <p className="hint">{data.verified_states} states verified by content hash · main → {data.branches.main?.slice(0, 19) ?? "—"}</p>}
        <table>
          <thead><tr><th>state</th><th>workflow</th><th>created (UTC)</th><th>result</th><th /></tr></thead>
          <tbody>{data?.history?.map((h: any) => (
            <tr key={h.hash}><td title={h.hash}><code>{h.hash.slice(7, 19)}</code>{h.on_main ? "" : " ·"}</td>
              <td>{h.workflow}</td><td>{h.created_at}</td><td>{summary(h)}</td>
              <td>{h.workflow === "quick" && <button disabled={busy} onClick={() => run("repro", h.hash)}>repro</button>}</td></tr>
          ))}</tbody>
        </table>
      </Card>
    </>
  );
}
