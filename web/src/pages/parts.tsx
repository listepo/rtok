// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";
import { useConnection, useSnapshot } from "../api/query";
import type { Snapshot } from "../api/snapshot.gen";
import { Loading } from "../states";
import { focusRing } from "../ui/cx";
import { Pill } from "../ui/Pill";
import { compact, pct } from "./format";
import type { CheckState } from "./model";

/** Renders `children` once a snapshot exists; the shell already shows the offline banner. */
export function WithSnapshot({ children }: { children: (snap: Snapshot) => ReactNode }) {
    const connection = useConnection();
    const { data } = useSnapshot();
    if (data) return children(data);
    return connection === "closed" ? null : <Loading />;
}

export function Toolbar({ children }: { children: ReactNode }) {
    return (
        <div className="glass flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2.5">{children}</div>
    );
}

export function Split({ list, detail }: { list: ReactNode; detail: ReactNode }) {
    return (
        <div className="grid min-w-0 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_380px]">
            {list}
            {detail}
        </div>
    );
}

export function Kv({ rows }: { rows: readonly (readonly [string, ReactNode])[] }) {
    return (
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
            {rows.map(([k, v]) => (
                <div key={k} className="contents">
                    <dt className="text-fg-subtle">{k}</dt>
                    <dd className="min-w-0 break-words">{v}</dd>
                </div>
            ))}
        </dl>
    );
}

export function Count({ children }: { children: ReactNode }) {
    return <span className="ml-auto text-2xs text-fg-subtle">{children}</span>;
}

export function PanelLink({ to, children }: { to: `/${string}`; children: ReactNode }) {
    return (
        <Link to={to} className={`${focusRing} rounded-sm text-accent-fg hover:underline`}>
            {children}
        </Link>
    );
}

const checkTone = { pass: "ok", warn: "warn", fail: "fail", skip: "muted" } as const;

export const CheckPill = ({ state }: { state: CheckState }) => (
    <Pill tone={checkTone[state]} dot={state !== "skip"}>
        {state === "skip" ? "n/a" : state}
    </Pill>
);

export const LivePill = ({ live }: { live: boolean }) => (
    <Pill tone={live ? "ok" : "muted"} dot={live}>
        {live ? "live" : "ended"}
    </Pill>
);

const surfaceTone = { hook: "info", mcp: "ok", proxy: "warn" } as const;

export const SurfacePill = ({ surface }: { surface: string }) => (
    <Pill tone={surfaceTone[surface as keyof typeof surfaceTone] ?? "muted"}>{surface}</Pill>
);

const levelTone = { error: "fail", warn: "warn", debug: "muted", info: "info" } as const;

export const LevelPill = ({ level }: { level: string }) => (
    <Pill tone={levelTone[level as keyof typeof levelTone] ?? "info"}>{level}</Pill>
);

interface Tokens {
    input: number;
    cache_create: number;
    cache_read: number;
    output: number;
}

export const tokenTotal = (t: Tokens) => t.input + t.cache_create + t.cache_read + t.output;

/** Stacked composition bar plus legend; the bar is a picture, so its label carries the shares. */
export function TokenMix({ tokens }: { tokens: Tokens }) {
    const parts: [string, number, string][] = [
        ["input", tokens.input, "bg-accent-fg"],
        ["cache create", tokens.cache_create, "bg-accent-fg/60"],
        ["cache read", tokens.cache_read, "bg-accent-fg/30"],
        ["output", tokens.output, "bg-delta"],
    ];
    const total = Math.max(1, tokenTotal(tokens));
    return (
        <>
            <div
                role="img"
                aria-label={parts.map(([k, v]) => `${k} ${pct(v / total, 0)}`).join(", ")}
                className="flex h-2 overflow-hidden rounded-full bg-surface-3"
            >
                {parts.map(([k, v, cls]) => (
                    <div key={k} className={cls} style={{ width: `${(v / total) * 100}%` }} />
                ))}
            </div>
            <ul className="flex flex-wrap gap-x-4 gap-y-1 text-2xs text-fg-muted">
                {parts.map(([k, v, cls]) => (
                    <li key={k} className="flex items-center gap-1.5">
                        <span aria-hidden="true" className={`size-2 rounded-full ${cls}`} />
                        {k} {compact(v)}
                    </li>
                ))}
            </ul>
        </>
    );
}
