import { useState } from "react";
import { api, fmt } from "../api";
import { Bars, Card, Err, KV } from "./common";
import { contractStore } from "./Noise";

export function ComparePanel() {
  const [shape, setShape] = useState({ m: 32, k: 128, n: 16 });
  const [seed, setSeed] = useState(0);
  const [gemm, setGemm] = useState<any>(null);
  const [text, setText] = useState("To be, or not to be, that is the question.");
  const [ll, setLl] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  const contract = contractStore.current ?? undefined;
  const runGemm = async () => {
    setError(null);
    try { setGemm(await api("/api/compare/gemm", { ...shape, seed, contract })); } catch (e) { setError(String(e)); }
  };
  const runLl = async () => {
    setError(null);
    try { setLl(await api("/api/models/likelihood", { text, noise: contract, seed })); } catch (e) { setError(String(e)); }
  };
  return (
    <>
      <Card title="GEMM: photonic simulator vs CPU">
        <p className="hint">Contract: {contract ? "the one validated in the Noise tab" : "the default (illustrative) contract"}.</p>
        <div className="row">
          {(["m", "k", "n"] as const).map((d) => (
            <label key={d}>{d} <input type="number" min={1} max={512} value={shape[d]}
                                      onChange={(e) => setShape({ ...shape, [d]: +e.target.value })} /></label>
          ))}
          <label>seed <input type="number" value={seed} onChange={(e) => setSeed(+e.target.value)} /></label>
          <button className="primary" onClick={runGemm}>run both</button>
        </div>
        <Err error={error} />
        {gemm && (
          <>
            <KV data={{ "relative RMS error": fmt(gemm.error.relative_rms), "max |error|": fmt(gemm.error.max_abs),
              "effective bits": fmt(gemm.error.effective_bits, 2), "CPU seconds (measured)": fmt(gemm.cpu.seconds),
              "simulator seconds (host)": fmt(gemm.photonic_sim.seconds) }} />
            <Bars counts={gemm.error.histogram.counts} edges={gemm.error.histogram.edges} />
            <p className="hint">{gemm.labels.seconds}</p>
          </>
        )}
      </Card>
      <Card title="AWEN-Mini likelihood: photonic simulator vs CPU">
        <textarea rows={2} value={text} onChange={(e) => setText(e.target.value)} />
        <button className="primary" onClick={runLl}>score</button>
        {ll && <KV data={{ "CPU nats/char (measured)": fmt(ll.cpu_nats), "photonic-sim nats/char (simulated)": fmt(ll.photonic_sim_nats),
          "Δ nats": fmt(ll.delta_nats) }} />}
      </Card>
    </>
  );
}
