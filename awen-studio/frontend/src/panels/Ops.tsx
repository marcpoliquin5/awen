import { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Err } from "./common";

export function OpsPanel() {
  const [data, setData] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { api("/api/ops").then(setData).catch((e) => setError(String(e))); }, []);
  return (
    <Card title="Operator library">
      <Err error={error} />
      {data && (
        <>
          <p className="hint">Levels: {Object.entries(data.levels).map(([k, v]) => `${k} ${v}`).join(" · ")}.
            No operator is hardware-measured: no photonic hardware is available.</p>
          <table>
            <thead><tr><th>op</th><th>level</th><th>inputs</th><th>params</th><th>photonic part</th><th>electronic part</th></tr></thead>
            <tbody>{data.ops.map((o: any) => (
              <tr key={o.name}><td>{o.name}</td><td><span className={`level ${o.level}`}>{o.level}</span></td>
                <td>{o.inputs.join(", ")}</td><td>{o.params.join(", ") || "—"}</td><td>{o.photonic}</td><td>{o.electronic}</td></tr>
            ))}</tbody>
          </table>
        </>
      )}
    </Card>
  );
}
