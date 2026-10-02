import type { CallRow, PluginPage, Snapshot } from "../api/snapshot.gen";

/** Σ tokens a call put through the model, `null` when the call carried no usage. */
export const tokensOf = (c: CallRow): number | null =>
  c.input == null ? null : c.input + (c.cache_create ?? 0) + (c.cache_read ?? 0) + (c.output ?? 0);

export const savedOf = (p: PluginPage): number | null =>
  p.stats ? p.stats.est_before - p.stats.est_after : null;

const sum = <T>(xs: readonly T[], f: (x: T) => number) => xs.reduce((s, x) => s + f(x), 0);

/** Everything the overview shows, derived once from one snapshot (admin.js `derive`). */
export function overview(snap: Snapshot) {
  const u = snap.usage;
  // A plugin without Measurement rows has no estimate; counting it would dilute the saving.
  const measured = snap.plugins.filter((p) => p.stats && p.stats.est_before > 0);
  const estBefore = sum(measured, (p) => p.stats?.est_before ?? 0);
  const estAfter = sum(measured, (p) => p.stats?.est_after ?? 0);
  const saved = estBefore - estAfter;
  const ctx = u.input + u.cache_create + u.cache_read;
  const ms = snap.calls.flatMap((c) => (c.ms == null ? [] : [c.ms])).sort((a, b) => a - b);
  const p95 = ms.length
    ? (ms[Math.min(ms.length - 1, Math.floor(0.95 * ms.length))] ?? null)
    : null;
  return {
    usage: u,
    ctx,
    measured: measured
      .map((plugin) => ({ plugin, saved: savedOf(plugin) ?? 0 }))
      .sort((a, b) => b.saved - a.saved),
    estBefore,
    estAfter,
    saved,
    deltaPct: estBefore ? saved / estBefore : NaN,
    cacheHit: ctx ? u.cache_read / ctx : NaN,
    failed: snap.calls.filter((c) => !c.ok).length,
    p95,
    live: snap.sessions.filter((s) => s.ended_at == null).length,
    hosts: new Set(snap.sessions.map((s) => s.host)).size,
    enabled: snap.plugins.filter((p) => p.enabled).length,
  };
}
