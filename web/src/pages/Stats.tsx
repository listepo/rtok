// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useMemo } from "react";
import { Empty } from "../states";
import { DataTable, type Column } from "../ui/DataTable";
import { Kpi } from "../ui/Kpi";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { compact, fmt } from "./format";
import { OtherLines, responsive, TextPage, useMinWidth, WithSnapshot } from "./parts";
import { parseStats, type Bust, type HealthRow, type PricedRow, type StatsView } from "./text";

// A missing key reads as unknown (`-`), never as NaN or 0.
const n = (v: string | undefined) => (v == null ? null : Number(v));
const money = (v: number | null) => (v == null ? "-" : v.toFixed(2));

/** Cost and saving totals: the report's own `cost total` line, else the sum of priced rows. */
export function costTotals(v: StatsView): { cost: number; saved: number } {
    if (v.costTotal) return v.costTotal;
    const sum = (f: (r: PricedRow) => number | null) =>
        v.priced.reduce((s, r) => s + (f(r) ?? 0), 0);
    return { cost: sum((r) => r.cost), saved: sum((r) => r.saved) };
}

export function Stats() {
    return (
        <WithSnapshot>
            {(snap) => (
                <TextPage
                    page="stats"
                    text={snap.stats}
                    command="rtok stats --price"
                    note="`rtok stats --price` plus `rtok stats --cache`, as one report"
                >
                    {(text) => <StatsBody view={parseStats(text)} />}
                </TextPage>
            )}
        </WithSnapshot>
    );
}

function StatsBody({ view: v }: { view: StatsView }) {
    const { totals: a, usage: u } = v;
    const t = costTotals(v);
    const maxCost = Math.max(1, ...v.priced.map((r) => r.cost ?? 0));

    const wide = useMinWidth(768);
    const priceFull = useMemo<Column<PricedRow>[]>(
        () => [
            { id: "model", header: "model", cell: (r) => <b>{r.model}</b> },
            {
                id: "input",
                header: "input",
                width: "64px",
                align: "right",
                cell: (r) => compact(r.input),
            },
            {
                id: "output",
                header: "output",
                width: "64px",
                align: "right",
                cell: (r) => compact(r.output),
            },
            {
                id: "cost",
                header: "cost $",
                width: "64px",
                align: "right",
                cell: (r) =>
                    r.cost == null ? (
                        <span className="text-fg-subtle" title="no price row">
                            -
                        </span>
                    ) : (
                        <b>{money(r.cost)}</b>
                    ),
            },
            {
                id: "saved",
                header: "saved $",
                width: "64px",
                align: "right",
                cell: (r) =>
                    r.saved == null ? (
                        <span className="text-fg-subtle">-</span>
                    ) : (
                        <span className="text-delta-fg">{money(r.saved)}</span>
                    ),
            },
            {
                id: "share",
                header: "share",
                width: "72px",
                cell: (r) => (
                    <div
                        aria-hidden="true"
                        className="h-1.5 overflow-hidden rounded-full bg-surface-3"
                    >
                        <div
                            className="h-full rounded-full bg-accent-fg"
                            style={{ width: `${(((r.cost ?? 0) / maxCost) * 100).toFixed(1)}%` }}
                        />
                    </div>
                ),
            },
        ],
        [maxCost],
    );
    const priceColumns = responsive(priceFull, wide, ["model", "cost", "saved"]);

    const healthFull = useMemo<Column<HealthRow>[]>(
        () => [
            { id: "session", header: "session", cell: (r) => <b>{r.session.slice(0, 13)}</b> },
            {
                id: "turns",
                header: "turns",
                width: "56px",
                align: "right",
                cell: (r) => fmt(r.turns),
            },
            {
                id: "read",
                header: "cache read",
                width: "88px",
                align: "right",
                cell: (r) => compact(r.cacheRead),
            },
            {
                id: "busts",
                header: "busts",
                width: "56px",
                align: "right",
                cell: (r) =>
                    r.busts ? (
                        <Pill tone="warn">{r.busts}</Pill>
                    ) : (
                        <span className="text-fg-subtle">0</span>
                    ),
            },
        ],
        [],
    );
    const healthColumns = responsive(healthFull, wide, ["session", "turns", "busts"]);

    return (
        <>
            <p className="text-2xs text-fg-subtle">
                {fmt(n(a.lines))} transcript lines · {a.malformed ?? "?"} malformed ·{" "}
                {a.no_checkpoint ?? "?"} sessions without checkpoint
            </p>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-6">
                <Kpi
                    label="sessions"
                    value={fmt(n(a.sessions))}
                    sub={`${a.compact ?? "?"} compact · ${a.checkpoint ?? "?"} checkpoint`}
                />
                <Kpi
                    label="cache hit"
                    value={u.hit ?? "-"}
                    sub={`read ${compact(n(u.cache_read))}`}
                />
                <Kpi
                    label="input"
                    value={compact(n(u.input))}
                    sub={`cache create ${compact(n(u.cache_create))}`}
                />
                <Kpi
                    label="output"
                    value={compact(n(u.output))}
                    sub={`median ctx ${compact(n(u.median_context))}`}
                />
                <Kpi label="cost" value={`$${t.cost.toFixed(2)}`} sub="at [stats.prices] $/MTok" />
                <Kpi
                    label="saved"
                    value={`$${t.saved.toFixed(2)}`}
                    tone="ok"
                    sub="cache reads priced"
                />
            </div>
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                <Panel title="cost per model" hint="stats --price">
                    <DataTable
                        label="cost per model"
                        rows={v.priced}
                        columns={priceColumns}
                        getRowId={(r) => r.model}
                        empty={
                            <Empty
                                title="No priced usage rows"
                                hint="Run the proxy to record usage."
                            />
                        }
                    />
                    <p className="text-2xs text-fg-subtle">
                        <code>-</code> = no price row; rtok never guesses a price
                    </p>
                </Panel>
                <Panel title="cache health" hint="stats --cache">
                    <DataTable
                        label="cache health"
                        rows={v.health}
                        columns={healthColumns}
                        getRowId={(r) => r.session}
                        empty={
                            <Empty title="No cache health rows" hint="Run `rtok proxy` first." />
                        }
                    />
                    {v.busts.length > 0 && <Busts busts={v.busts} />}
                </Panel>
            </div>
            <OtherLines lines={v.other} />
        </>
    );
}

function Busts({ busts }: { busts: Bust[] }) {
    return (
        <ul aria-label="cache busts" className="flex flex-col divide-y divide-border/60 text-2xs">
            {busts.map((b, i) => (
                <li key={`${b.turn}-${i}`} className="flex flex-wrap items-center gap-2 py-2">
                    <Pill tone="warn">bust</Pill>
                    <span>turn {b.turn}</span>
                    <span className="text-fg-muted">
                        cause <b className="text-fg">{b.cause}</b>
                    </span>
                    <span className="ml-auto text-fg-muted">
                        cache create {compact(b.cacheCreate)} · read {compact(b.cacheRead)}
                    </span>
                </li>
            ))}
        </ul>
    );
}
