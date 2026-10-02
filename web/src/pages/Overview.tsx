import { useMemo, type ReactNode } from "react";
import { Empty } from "../states";
import { DataTable, type Column } from "../ui/DataTable";
import { Kpi } from "../ui/Kpi";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Bitset, MiniBars } from "../ui/Marks";
import { Sparkline } from "../ui/Sparkline";
import { compact, fmt, pct } from "./format";
import { overview } from "./model";
import { CallsPanel, DoctorPanel, SessionsPanel } from "./OverviewPanels";
import { PanelLink, WithSnapshot } from "./parts";

type Saving = ReturnType<typeof overview>["measured"][number];

function savingColumns(max: number): Column<Saving>[] {
    return [
        { id: "plugin", header: "plugin", cell: (r) => <b>{r.plugin.id}</b> },
        {
            id: "rows",
            header: "rows",
            width: "64px",
            align: "right",
            cell: (r) => fmt(r.plugin.stats?.rows),
        },
        {
            id: "saved",
            header: "saved",
            width: "72px",
            align: "right",
            cell: (r) => compact(r.saved),
        },
        { id: "share", header: "share", cell: (r) => <Share value={r.saved} max={max} /> },
    ];
}

function Share({ value, max }: { value: number; max: number }) {
    return (
        <div className="h-1.5 overflow-hidden rounded-full bg-surface-3">
            <div
                className="h-full rounded-full bg-delta-fg/80"
                style={{ width: `${max ? (value / max) * 100 : 0}%` }}
            />
        </div>
    );
}

const sub = (text: string) => <span className="text-sm text-fg-subtle">{text}</span>;

export function Overview() {
    return <WithSnapshot>{(snap) => <OverviewBody snap={snap} />}</WithSnapshot>;
}

function OverviewBody({ snap }: { snap: Parameters<typeof overview>[0] }) {
    const o = overview(snap);
    const u = o.usage;
    const max = Math.max(1, ...o.measured.map((m) => m.saved));
    const columns = useMemo(() => savingColumns(max), [max]);
    const mix: [string, number, string][] = [
        ["input", u.input, "bg-accent-fg"],
        ["cache create", u.cache_create, "bg-accent-fg/60"],
        ["cache read", u.cache_read, "bg-accent-fg/30"],
        ["output", u.output, "bg-delta"],
    ];
    const mixTotal = Math.max(
        1,
        mix.reduce((s, m) => s + m[1], 0),
    );
    const kpis: ReactNode[] = [
        <Kpi
            key="in"
            label="input tok"
            value={compact(u.input)}
            sub={`ctx ${compact(o.ctx)} incl. cache`}
            viz={<Sparkline values={u.turns.slice(-40)} label="ctx tokens per turn" />}
        />,
        <Kpi
            key="out"
            label="output tok"
            value={compact(u.output)}
            sub={`cache create ${compact(u.cache_create)}`}
        />,
        <Kpi
            key="saved"
            label="Δ saved tok"
            value={
                <>
                    <span className="text-delta-fg">Δ</span> {compact(o.saved)}
                </>
            }
            sub={`est ${compact(o.estBefore)} → ${compact(o.estAfter)}`}
            viz={
                <MiniBars
                    values={o.measured.slice(0, 8).map((m) => m.saved)}
                    label="saved per plugin"
                    className="fill-delta-fg"
                />
            }
        />,
        <Kpi
            key="pct"
            label="Δtok %"
            value={<span className="text-accent-fg">{pct(o.deltaPct)}</span>}
            sub={`${o.measured.length} plugins with Measurement rows`}
        />,
        <Kpi
            key="cache"
            label="cache hit"
            value={pct(o.cacheHit)}
            sub={`read ${compact(u.cache_read)} of ctx`}
        />,
        <Kpi
            key="calls"
            label="calls"
            value={fmt(snap.calls.length)}
            sub={`${o.failed} failed · p95 ${o.p95 == null ? "-" : `${o.p95.toFixed(0)} ms`}`}
            viz={
                <MiniBars
                    values={o.buckets.map((b) => b.hook + b.mcp + b.proxy)}
                    label="calls per bucket"
                />
            }
        />,
        <Kpi
            key="live"
            label="live sessions"
            value={
                <>
                    {o.live}
                    {sub(` / ${snap.sessions.length}`)}
                </>
            }
            sub={`${o.hosts} hosts`}
            viz={<Sparkline values={o.liveSeries} label="sessions alive over the calls window" />}
        />,
        <Kpi
            key="on"
            label="plugins on"
            value={
                <>
                    {o.enabled}
                    {sub(` / ${snap.plugins.length}`)}
                </>
            }
            sub={`${snap.plugins.length - o.enabled} disabled`}
            viz={<Bitset items={snap.plugins} />}
        />,
    ];
    return (
        <div className="flex flex-col gap-3">
            {(u.alerts ?? []).map((a) => (
                <div
                    key={a}
                    role="alert"
                    className="glass flex items-center gap-2 border-warn/50 px-3 py-2 text-xs"
                >
                    <Pill tone="warn">alert</Pill>
                    {a}
                </div>
            ))}
            <div className="grid grid-cols-2 gap-3 md:grid-cols-4">{kpis}</div>
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-12">
                <CallsPanel o={o} calls={snap.calls.length} />
                <Panel
                    title="token mix"
                    hint={`${fmt(mixTotal)} tokens in the ledger window`}
                    className="xl:col-span-4"
                >
                    <div
                        role="img"
                        aria-label={mix.map(([k, v]) => `${k} ${pct(v / mixTotal, 0)}`).join(", ")}
                        className="flex h-2 overflow-hidden rounded-full bg-surface-3"
                    >
                        {mix.map(([k, v, cls]) => (
                            <div
                                key={k}
                                className={cls}
                                style={{ width: `${(v / mixTotal) * 100}%` }}
                            />
                        ))}
                    </div>
                    <ul className="flex flex-wrap gap-x-4 gap-y-1 text-2xs text-fg-muted">
                        {mix.map(([k, v, cls]) => (
                            <li key={k} className="flex items-center gap-1.5">
                                <span aria-hidden="true" className={`size-2 rounded-full ${cls}`} />
                                {k} {compact(v)}
                            </li>
                        ))}
                    </ul>
                </Panel>
                <Panel
                    title="savings by plugin"
                    hint="Σ est before − after, measured plugins only"
                    action={<PanelLink to="/plugins">plugins →</PanelLink>}
                    className="xl:col-span-7"
                >
                    <DataTable
                        label="savings by plugin"
                        rows={o.measured}
                        columns={columns}
                        getRowId={(r) => r.plugin.id}
                        height={Math.min(320, 36 * Math.max(1, o.measured.length))}
                        empty={
                            <Empty
                                title="No measured savings yet"
                                hint="Plugins fill this in as they record Measurement rows."
                            />
                        }
                    />
                </Panel>
                <DoctorPanel o={o} />
                <SessionsPanel o={o} />
            </div>
        </div>
    );
}
