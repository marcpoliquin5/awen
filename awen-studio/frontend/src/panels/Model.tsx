import { useEffect, useState } from "react";
import { api, fmt } from "../api";
import { Card, Err, KV } from "./common";

export function ModelPanel() {
  const [models, setModels] = useState<any[]>([]);
  const [prompt, setPrompt] = useState("ROMEO:");
  const [device, setDevice] = useState("cpu");
  const [tokens, setTokens] = useState(120);
  const [temperature, setTemperature] = useState(0.8);
  const [seed, setSeed] = useState(0);
  const [out, setOut] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { api("/api/models").then((d) => setModels(d.models)).catch((e) => setError(String(e))); }, []);

  const run = async () => {
    setBusy(true); setError(null);
    try {
      setOut(await api("/api/models/generate", { prompt, device, max_new_tokens: tokens, temperature, seed }));
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };
  const m = models[0];
  return (
    <>
      <Card title="AWEN-Mini" tag="measured">
        {m ? <KV data={{ parameters: m.parameters, dataset: m.dataset, "val loss (nats)": m.final?.val_loss,
          "trained on": m.hardware?.cpu_model, "training seconds": m.final?.elapsed_s,
          "weights sha256": m.weights_sha256.slice(0, 16) + "…" }} /> : <p>loading…</p>}
        <p className="hint">A 1M-parameter character-level transformer trained on CPU. It writes Shakespeare-flavoured
          text; it is a test model for the toolchain, not an assistant.</p>
      </Card>
      <Card title="Generate">
        <textarea value={prompt} onChange={(e) => setPrompt(e.target.value)} rows={2} />
        <div className="row">
          <label>device
            <select value={device} onChange={(e) => setDevice(e.target.value)}>
              <option value="cpu">cpu (PyTorch)</option>
              <option value="photonic-sim">photonic-sim (every linear layer on the simulator)</option>
            </select>
          </label>
          <label>tokens <input type="number" min={1} max={400} value={tokens} onChange={(e) => setTokens(+e.target.value)} /></label>
          <label>temperature <input type="number" step={0.1} min={0} max={2} value={temperature} onChange={(e) => setTemperature(+e.target.value)} /></label>
          <label>seed <input type="number" value={seed} onChange={(e) => setSeed(+e.target.value)} /></label>
          <button className="primary" disabled={busy} onClick={run}>{busy ? "running…" : "generate"}</button>
        </div>
        <Err error={error} />
        {out && (
          <>
            <pre className="gen"><b>{prompt}</b>{out.completion}</pre>
            <KV data={{ device: out.execution.device, fidelity: out.execution.fidelity,
              "gemm backend": out.execution.gemm_backend, seconds: fmt(out.seconds),
              "chars/s (host)": fmt(out.tokens_per_second, 1) }} />
          </>
        )}
      </Card>
    </>
  );
}
