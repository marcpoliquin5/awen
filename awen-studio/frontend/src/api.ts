// Thin client for the local AWEN Studio API (same origin, /api/...).
export type Json = Record<string, unknown>;

export async function api<T = any>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  let data: any = text;
  try {
    data = JSON.parse(text);
  } catch {
    /* plain-text endpoints (report.md) */
  }
  if (!response.ok) {
    const detail = typeof data === "object" && data && "detail" in data ? data.detail : text;
    throw new Error(`${response.status}: ${typeof detail === "string" ? detail : JSON.stringify(detail)}`);
  }
  return data as T;
}

export function download(name: string, content: string, type: string) {
  const url = URL.createObjectURL(new Blob([content], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

export const fmt = (v: unknown, digits = 4): string => {
  if (typeof v === "number") {
    if (v === 0) return "0";
    const a = Math.abs(v);
    return a >= 1e4 || a < 1e-3 ? v.toExponential(2) : v.toFixed(digits).replace(/\.?0+$/, "");
  }
  return v === null || v === undefined ? "—" : String(v);
};
