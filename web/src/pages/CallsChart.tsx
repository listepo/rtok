// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { focusRing } from "../ui/cx";
import { hm } from "./format";
import { SURFACES, type Bucket, type Surface } from "./model";

// Ported from `callsChart()` in design/html/js/admin.js.
const W = 720;
const H = 180;
const PAD = { l: 32, r: 8, t: 8, b: 22 };

export const SERIES: Record<Surface, { swatch: string; fill: string }> = {
    hook: { swatch: "bg-accent-fg", fill: "fill-accent-fg" },
    mcp: { swatch: "bg-accent-fg/50", fill: "fill-accent-fg/50" },
    proxy: { swatch: "bg-fg-muted/75", fill: "fill-fg-muted/75" },
};

export function CallsChart({
    buckets,
    step,
    failed,
}: {
    buckets: readonly Bucket[];
    step: number;
    failed: number;
}) {
    const total = (b: Bucket) => b.hook + b.mcp + b.proxy;
    const max = Math.max(1, ...buckets.map(total));
    const top = Math.ceil(max / 4) * 4;
    const inner = { w: W - PAD.l - PAD.r, h: H - PAD.t - PAD.b };
    const bar = inner.w / buckets.length;
    const y = (v: number) => PAD.t + inner.h - (v / top) * inner.h;
    const all = buckets.reduce((s, b) => s + total(b), 0);
    return (
        // The 9px axis text is unreadable once the chart shrinks to a phone, so it scrolls instead.
        <div tabIndex={0} className={`overflow-x-auto rounded-md ${focusRing}`}>
            <svg
                viewBox={`0 0 ${W} ${H}`}
                className="h-auto w-full min-w-[520px]"
                role="img"
                aria-label={`Calls over time: ${all} calls in ${buckets.length} buckets of ${Math.max(1, Math.round(step / 60))} minutes, ${failed} failed`}
            >
                {[0, 1, 2, 3, 4].map((i) => {
                    const v = (top / 4) * i;
                    return (
                        <g key={i}>
                            <line
                                x1={PAD.l}
                                x2={W - PAD.r}
                                y1={y(v)}
                                y2={y(v)}
                                className="stroke-border"
                                strokeDasharray={i ? "2 3" : undefined}
                            />
                            <text
                                x={PAD.l - 6}
                                y={y(v) + 3}
                                textAnchor="end"
                                fontSize="9"
                                className="fill-fg-muted"
                            >
                                {v}
                            </text>
                        </g>
                    );
                })}
                {buckets.map((b, i) => {
                    let acc = 0;
                    return (
                        <g key={b.t}>
                            {SURFACES.map((s) => {
                                const v = b[s];
                                if (!v) return null;
                                const rect = (
                                    <rect
                                        key={s}
                                        x={PAD.l + i * bar + 2}
                                        y={y(acc + v)}
                                        width={bar - 4}
                                        height={y(acc) - y(acc + v)}
                                        className={SERIES[s].fill}
                                    >
                                        <title>{`${hm(b.t)} · ${s} ${v}`}</title>
                                    </rect>
                                );
                                acc += v;
                                return rect;
                            })}
                            {b.err > 0 && (
                                <circle
                                    cx={PAD.l + i * bar + bar / 2}
                                    cy={y(acc) - 6}
                                    r="2.5"
                                    className="fill-delta-fg"
                                >
                                    <title>{`${b.err} failed`}</title>
                                </circle>
                            )}
                            {i % 4 === 0 && (
                                <text
                                    x={PAD.l + i * bar}
                                    y={H - 6}
                                    fontSize="9"
                                    className="fill-fg-muted"
                                >
                                    {hm(b.t)}
                                </text>
                            )}
                        </g>
                    );
                })}
            </svg>
        </div>
    );
}
