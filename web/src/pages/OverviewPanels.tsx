// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { Empty } from "../states";
import { Panel } from "../ui/Panel";
import { SERIES, CallsChart } from "./CallsChart";
import { ago, compact, nowSecs } from "./format";
import { SURFACES, type overview } from "./model";
import { CheckPill, LivePill, PanelLink } from "./parts";

type Overview = ReturnType<typeof overview>;

export function CallsPanel({ o, calls }: { o: Overview; calls: number }) {
    return (
        <Panel
            title="calls over time"
            hint={`${calls} rows · ${Math.max(1, Math.round(o.step / 60))} min buckets`}
            action={<PanelLink to="/calls">calls →</PanelLink>}
            className="xl:col-span-8"
        >
            {calls ? (
                <>
                    <CallsChart buckets={o.buckets} step={o.step} failed={o.failed} />
                    <ul className="flex flex-wrap items-center gap-x-4 gap-y-1 text-2xs text-fg-muted">
                        {SURFACES.map((s) => (
                            <li key={s} className="flex items-center gap-1.5">
                                <span
                                    aria-hidden="true"
                                    className={`size-2.5 rounded-sm ${SERIES[s].swatch}`}
                                />
                                {s} {o.bySurface[s].n}
                            </li>
                        ))}
                        <li className="flex items-center gap-1.5">
                            <span aria-hidden="true" className="size-2 rounded-full bg-delta-fg" />
                            failed {o.failed}
                        </li>
                        <li className="ml-auto">
                            p50 {o.p50 == null ? "-" : o.p50.toFixed(1)} ms · p95{" "}
                            {o.p95 == null ? "-" : o.p95.toFixed(0)} ms
                        </li>
                    </ul>
                </>
            ) : (
                <Empty
                    title="No calls yet"
                    hint="The ledger fills as hooks, MCP and the proxy run."
                />
            )}
        </Panel>
    );
}

export function DoctorPanel({ o }: { o: Overview }) {
    const count = (st: string) => o.checks.filter((c) => c.st === st).length;
    const open = o.checks.filter((c) => c.st === "warn" || c.st === "fail").slice(0, 5);
    return (
        <Panel
            title="doctor"
            action={<PanelLink to="/doctor">doctor →</PanelLink>}
            className="xl:col-span-5"
        >
            <div className="grid grid-cols-3 gap-2 text-center">
                {(
                    [
                        ["pass", "text-success-fg"],
                        ["warn", "text-warn-fg"],
                        ["fail", "text-delta-fg"],
                    ] as const
                ).map(([st, tone]) => (
                    <div key={st} className="rounded-md bg-surface-2 py-2">
                        <div className={`text-lg font-semibold ${tone}`}>{count(st)}</div>
                        <div className="text-2xs tracking-kicker text-fg-muted uppercase">{st}</div>
                    </div>
                ))}
            </div>
            <ul className="flex flex-col divide-y divide-border/60">
                {open.length ? (
                    open.map((c) => (
                        <li key={c.label + c.detail} className="flex items-start gap-2 py-2">
                            <CheckPill state={c.st} />
                            <div className="min-w-0">
                                <div className="text-xs font-semibold">{c.label}</div>
                                <div className="text-2xs break-words text-fg-muted">{c.detail}</div>
                            </div>
                        </li>
                    ))
                ) : (
                    <li className="py-2 text-xs text-fg-muted">All checks pass</li>
                )}
            </ul>
        </Panel>
    );
}

export function SessionsPanel({ o }: { o: Overview }) {
    return (
        <Panel
            title="recent sessions"
            hint={`${o.live} live`}
            action={<PanelLink to="/sessions">all →</PanelLink>}
            className="xl:col-span-12"
        >
            {o.recent.length ? (
                <ul className="-m-4 divide-y divide-border/60">
                    {o.recent.map((s) => (
                        <li key={s.id} className="flex items-center gap-3 px-4 py-2">
                            <LivePill live={s.ended_at == null} />
                            <div className="min-w-0 flex-1">
                                <div className="truncate text-xs font-semibold">
                                    {s.id.slice(0, 8)}{" "}
                                    <span className="font-normal text-fg-muted">
                                        {s.host ?? "-"} · {s.model ?? "-"}
                                    </span>
                                </div>
                                <div className="truncate text-2xs text-fg-muted">
                                    {s.project ?? "-"}
                                </div>
                            </div>
                            <div className="shrink-0 text-right">
                                <div className="text-xs">
                                    {compact(s.input + s.cache_create + s.cache_read + s.output)}
                                </div>
                                <div className="text-2xs text-fg-muted">
                                    {ago(s.last_activity, nowSecs())}
                                </div>
                            </div>
                        </li>
                    ))}
                </ul>
            ) : (
                <Empty title="No sessions yet" hint="A session appears with its first hook." />
            )}
        </Panel>
    );
}
