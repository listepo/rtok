// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useMemo } from "react";
import type { UsagePage } from "../api/snapshot.gen";
import { Empty } from "../states";
import { DataTable, type Column } from "../ui/DataTable";
import { Kpi } from "../ui/Kpi";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { compact, fmt } from "./format";
import { responsive, useMinWidth, WithSnapshot } from "./parts";
import type { UsageReport } from "./model";

/** Dollars like the CLI prints them; no price is `-`, never `$0.00` (that would mean free). */
export function usd(cost: number | null | undefined): string {
    if (cost == null) return "-";
    const cents = Math.round(Math.abs(cost) * 100);
    const sign = cost < 0 && cents > 0 ? "-" : "";
    return `${sign}$${fmt(Math.floor(cents / 100))}.${String(cents % 100).padStart(2, "0")}`;
}

const sources = {
    logs: "agent logs",
    rtok: "through rtok",
    both: "agent logs and through rtok",
} as const;

/** One table line, whether the report grouped by agent or (`--by model`) by model. */
export interface Line {
    id: string;
    label: string;
    tokens: number;
    cost: number | null;
    throughRtok: number | null;
    coverage: number | null;
    savedTokens: number | null;
    savedUsd: number | null;
}

/** The middle table's rows: the models when the report asked for them, else the agents. */
export function usageLines(r: UsageReport): Line[] {
    if (r.models) {
        return r.models.map((m) => ({
            id: m.model,
            label: m.model,
            tokens: m.tokens,
            cost: m.cost_usd,
            throughRtok: null,
            coverage: null,
            savedTokens: null,
            savedUsd: null,
        }));
    }
    return r.agents.map((a) => ({
        id: a.host,
        label: a.name,
        tokens: a.tokens,
        cost: a.cost_usd,
        throughRtok: a.through_rtok_tokens ?? null,
        coverage: a.coverage ?? null,
        savedTokens: a.saved_tokens ?? null,
        savedUsd: a.saved_usd ?? null,
    }));
}

/** Tokens the estimate leaves out because their model has no price. */
export const unpricedTokens = (r: UsageReport): number =>
    r.unpriced.reduce((sum, u) => sum + u.tokens, 0);

/** The report keeps no flag for `--daily`: a day period is a full date, a month is not. */
export const periodKind = (r: UsageReport): "Daily" | "Monthly" =>
    r.periods.some((p) => /^\d{4}-\d{2}-\d{2}$/.test(p.period)) ? "Daily" : "Monthly";

const pct = (c: number | null) => (c == null ? "-" : `${Math.round(c * 100)}%`);

export function Usage() {
    return <WithSnapshot>{(snap) => <UsageBody page={snap.agent_usage} />}</WithSnapshot>;
}

function UsageBody({ page }: { page: UsagePage }) {
    const { report, text } = page;
    if (!report) {
        // The server's own sentence: it is reading the logs, or says why it could not.
        const reading = text.startsWith("reading usage");
        return (
            <Panel title="usage" hint="rtok agents usage">
                <p role={reading ? "status" : "alert"} className="text-xs text-fg-muted">
                    {text.trim()}
                </p>
            </Panel>
        );
    }
    if (report.through == null) {
        return <Empty title="No usage recorded" hint={text.trim()} />;
    }
    return <Report r={report} />;
}

function Report({ r }: { r: UsageReport }) {
    const t = r.totals;
    const lines = useMemo(() => usageLines(r), [r]);
    const wide = useMinWidth(768);
    const both = r.source === "both";
    const saved = r.source !== "logs";
    const byModel = r.models != null;
    const incomplete = r.unpriced_models > 0;
    const kind = periodKind(r);
    const maxPeriod = Math.max(1, ...r.periods.map((p) => p.tokens));

    const lineFull = useMemo<Column<Line>[]>(() => {
        const cols: Column<Line>[] = [
            { id: "label", header: byModel ? "model" : "agent", cell: (l) => <b>{l.label}</b> },
            {
                id: "tokens",
                header: "tokens",
                width: "80px",
                align: "right",
                cell: (l) => <span title={fmt(l.tokens)}>{compact(l.tokens)}</span>,
            },
            {
                id: "cost",
                header: "est. cost",
                width: "96px",
                align: "right",
                cell: (l) =>
                    l.cost == null ? (
                        <span className="text-fg-subtle" title="no price in [stats.prices]">
                            -
                        </span>
                    ) : (
                        <b>{usd(l.cost)}</b>
                    ),
            },
        ];
        if (!byModel && both) {
            cols.push(
                {
                    id: "through",
                    header: "via rtok",
                    width: "80px",
                    align: "right",
                    cell: (l) => compact(l.throughRtok),
                },
                {
                    id: "coverage",
                    header: "coverage",
                    width: "80px",
                    align: "right",
                    cell: (l) => pct(l.coverage),
                },
            );
        }
        if (!byModel && saved) {
            cols.push(
                {
                    id: "saved",
                    header: "saved",
                    width: "80px",
                    align: "right",
                    cell: (l) => compact(l.savedTokens),
                },
                {
                    id: "savedUsd",
                    header: "saved est.",
                    width: "96px",
                    align: "right",
                    cell: (l) => <span className="text-delta-fg">{usd(l.savedUsd)}</span>,
                },
            );
        }
        return cols;
    }, [both, byModel, saved]);
    const lineColumns = responsive(lineFull, wide, ["label", "tokens", "cost"]);

    const periodFull = useMemo<Column<UsageReport["periods"][number]>[]>(
        () => [
            {
                id: "period",
                header: kind === "Daily" ? "day" : "month",
                cell: (p) => <b>{p.period}</b>,
            },
            {
                id: "tokens",
                header: "tokens",
                width: "80px",
                align: "right",
                cell: (p) => <span title={fmt(p.tokens)}>{compact(p.tokens)}</span>,
            },
            {
                id: "cost",
                header: "est. cost",
                width: "96px",
                align: "right",
                cell: (p) =>
                    p.cost_usd == null ? (
                        <span className="text-fg-subtle">-</span>
                    ) : (
                        usd(p.cost_usd)
                    ),
            },
            {
                id: "share",
                header: "share",
                width: "96px",
                cell: (p) => (
                    <div
                        aria-hidden="true"
                        className="h-1.5 overflow-hidden rounded-full bg-surface-3"
                    >
                        <div
                            className="h-full rounded-full bg-accent-fg"
                            style={{ width: `${((p.tokens / maxPeriod) * 100).toFixed(1)}%` }}
                        />
                    </div>
                ),
            },
        ],
        [kind, maxPeriod],
    );
    const periodColumns = responsive(periodFull, wide, ["period", "tokens", "cost"]);

    return (
        <div className="flex flex-col gap-3">
            <p className="text-2xs text-fg-subtle">
                {sources[r.source as keyof typeof sources] ?? r.source} · up to {r.through} ({r.tz})
            </p>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-5">
                <Kpi
                    label="tokens"
                    value={compact(t.tokens)}
                    sub={`in ${compact(t.input)} · out ${compact(t.output)} · cache ${compact(t.cache_read + t.cache_write)}`}
                />
                <Kpi
                    label="estimated cost"
                    value={usd(t.cost_usd)}
                    tone={incomplete ? "warn" : "default"}
                    sub={
                        incomplete
                            ? `incomplete: ${r.unpriced_models} unpriced`
                            : "at [stats.prices] $/MTok"
                    }
                />
                <Kpi label="sessions" value={fmt(t.sessions)} />
                <Kpi label="daily rows" value={fmt(t.daily_rows)} sub="agent-days with usage" />
                {t.saved_tokens != null && (
                    <Kpi
                        label="rtok saved"
                        value={compact(t.saved_tokens)}
                        tone="ok"
                        sub={
                            t.saved_usd == null
                                ? "no price for an estimate"
                                : `≈ ${usd(t.saved_usd)}`
                        }
                    />
                )}
            </div>
            {incomplete && (
                <Panel title="cost is incomplete" hint="rtok agents usage --unpriced">
                    <p role="alert" className="text-xs text-warn-fg">
                        {r.unpriced_models === 1
                            ? "1 model has"
                            : `${r.unpriced_models} models have`}{" "}
                        no price in [stats.prices], so {compact(unpricedTokens(r))} tokens are not
                        in the estimate.
                    </p>
                    <ul
                        aria-label="unpriced models"
                        className="flex flex-col divide-y divide-border/60 text-2xs"
                    >
                        {r.unpriced.map((u) => (
                            <li
                                key={`${u.host}-${u.model}`}
                                className="flex flex-wrap items-center gap-2 py-2"
                            >
                                <Pill tone="warn">no price</Pill>
                                <b>{u.model}</b>
                                <span className="text-fg-muted">{u.host}</span>
                                <span className="ml-auto text-fg-muted">
                                    {compact(u.tokens)} tokens
                                </span>
                            </li>
                        ))}
                    </ul>
                </Panel>
            )}
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                <Panel title={byModel ? "per model" : "per agent"} hint="agents usage">
                    <DataTable
                        label={byModel ? "usage per model" : "usage per agent"}
                        rows={lines}
                        columns={lineColumns}
                        getRowId={(l) => l.id}
                        empty={<Empty title="No usage rows" hint="Run an agent first." />}
                    />
                </Panel>
                <Panel
                    title={`${kind.toLowerCase()} totals`}
                    hint={kind === "Daily" ? "--daily" : "--monthly"}
                >
                    <DataTable
                        label={`${kind.toLowerCase()} usage`}
                        rows={r.periods}
                        columns={periodColumns}
                        getRowId={(p) => p.period}
                        empty={<Empty title="No periods" hint="Nothing falls in the window." />}
                    />
                </Panel>
            </div>
            {r.skipped.length > 0 && (
                <Panel title="skipped hosts" hint="files exist but could not be read">
                    <ul
                        aria-label="unreadable hosts"
                        className="flex flex-col divide-y divide-border/60 text-2xs"
                    >
                        {r.skipped.map((s) => (
                            <li
                                key={`${s.host}-${s.path}`}
                                className="flex flex-wrap items-center gap-2 py-2"
                            >
                                <Pill tone="warn">{s.reason}</Pill>
                                <b>{s.host}</b>
                                <code className="ml-auto text-fg-muted">{s.path}</code>
                            </li>
                        ))}
                    </ul>
                </Panel>
            )}
        </div>
    );
}
