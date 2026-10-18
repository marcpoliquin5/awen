import { useEffect, useState } from "react";
import { api, fmt } from "../api";
import { Card, Err, KV } from "./common";

export const contractStore: { current: Record<string, number> | null } = { current: null };

export function NoisePanel() {
  const [contract, setContract] = useState<Record<string, any> | null>(null);
  const [k, setK] = useState(64);
  const [result, setResult] = useState<any>(null);
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api("/api/noise/default").then((d) => { setContract(contractStore.current ?? d.default); setNote(d.note); })
      .catch((e) => setError(String(e)));
  }, []);
  const validate = async () => {
    setError(null);
    try {
      const r = await api("/api/noise/validate", { contract, k });
      setResult(r);
      contractStore.current = r.contract;
    } catch (e) { setError(String(e)); }
  };
  if (!contract) return <Card title="Noise contract"><Err error={error} /><p>loading…</p></Card>;
  return (
    <>
      <Card title="Noise contract" tag="simulated">
        <p className="hint">{note} The validated contract is used by the Compare panel.</p>
        <div className="grid">
          {Object.entries(contract).filter(([key]) => key !== "version").map(([key, value]) => (
            <label key={key}>{key}
              <input type="number" step="any" value={value as number}
                     onChange={(e) => setContract({ ...contract, [key]: e.target.value === "" ? "" : Number(e.target.value) })} />
            </label>
          ))}
          <label>tile length k <input type="number" min={1} value={k} onChange={(e) => setK(+e.target.value)} /></label>
        </div>
        <button className="primary" onClick={validate}>validate and predict</button>
        <Err error={error} />
      </Card>
      {result && (
        <Card title={`Analytic noise for one tile (k = ${result.k})`} tag="simulated">
          <KV data={Object.fromEntries(Object.entries(result.predicted).map(([a, b]) => [a, fmt(b)]))} />
        </Card>
      )}
    </>
  );
}
