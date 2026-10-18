import { ReactNode } from "react";
import { fmt } from "../api";

export function Card({ title, children, tag }: { title: string; children: ReactNode; tag?: string }) {
  return (
    <section className="card">
      <h2>{title} {tag && <span className={`tag ${tag}`}>{tag}</span>}</h2>
      {children}
    </section>
  );
}

export function Err({ error }: { error: string | null }) {
  return error ? <p className="bad">{error}</p> : null;
}

export function KV({ data }: { data: Record<string, unknown> }) {
  return (
    <dl className="kv">
      {Object.entries(data).map(([k, v]) => (
        <div key={k}><dt>{k}</dt><dd>{typeof v === "object" && v !== null ? JSON.stringify(v) : fmt(v)}</dd></div>
      ))}
    </dl>
  );
}

export function Bars({ counts, edges }: { counts: number[]; edges: number[] }) {
  const max = Math.max(1, ...counts);
  return (
    <div className="bars" title="error histogram (photonic-sim minus CPU)">
      {counts.map((c, i) => (
        <div key={i} className="bar" style={{ height: `${(100 * c) / max}%` }}
             title={`${fmt(edges[i])} … ${fmt(edges[i + 1])}: ${c}`} />
      ))}
    </div>
  );
}
