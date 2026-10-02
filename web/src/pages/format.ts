// Number and time formatting shared by the pages; ported from admin.js in design/html.
const nf = new Intl.NumberFormat("en-US");

export const fmt = (n: number | null | undefined): string => (n == null ? "-" : nf.format(n));

export function compact(n: number | null | undefined): string {
  if (n == null || !Number.isFinite(n)) return "-";
  const a = Math.abs(n);
  if (a >= 1e9) return `${(n / 1e9).toFixed(a >= 1e10 ? 0 : 1)}B`;
  if (a >= 1e6) return `${(n / 1e6).toFixed(a >= 1e7 ? 0 : 1)}M`;
  if (a >= 1e4) return `${(n / 1e3).toFixed(0)}k`;
  if (a >= 1e3) return `${(n / 1e3).toFixed(1)}k`;
  return String(n);
}

export const pct = (x: number, digits = 1): string =>
  Number.isFinite(x) ? `${(x * 100).toFixed(digits)}%` : "-";

const pad = (n: number) => String(n).padStart(2, "0");

/** Local wall-clock time of a unix-seconds timestamp. */
export function hms(ts: number): string {
  const d = new Date(ts * 1000);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export const iso = (ts: number): string => new Date(ts * 1000).toISOString().replace(".000", "");

/** Hour and minute of a unix-seconds timestamp, for chart axes. */
export const hm = (ts: number): string => hms(ts).slice(0, 5);

/** "5m ago" style age; `now` is a parameter so tests do not depend on the clock. */
export function ago(ts: number, now: number): string {
  const s = Math.max(0, now - ts);
  if (s < 60) return `${Math.floor(s)}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}

export const nowSecs = (): number => Math.floor(Date.now() / 1000);
