// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useMemo, useState } from "react";
import type { SessionTotals, Snapshot } from "../api/snapshot.gen";
import { Empty } from "../states";
import { DataTable, type Column } from "../ui/DataTable";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Search } from "../ui/Search";
import { Switch } from "../ui/Switch";
import { ago, compact, hms, iso, nowSecs } from "./format";
import { matchesSession } from "./model";
import {
    Count,
    Kv,
    LivePill,
    Split,
    SurfacePill,
    TokenMix,
    tokenTotal,
    Toolbar,
    WithSnapshot,
} from "./parts";

const columns: Column<SessionTotals>[] = [
    {
        id: "status",
        header: "status",
        width: "64px",
        cell: (s) => <LivePill live={s.ended_at == null} />,
    },
    {
        id: "id",
        header: "session",
        cell: (s) => (
            <span className="flex flex-col leading-tight">
                <b>{s.id.slice(0, 8)}</b>
                <span className="truncate text-2xs text-fg-muted">{s.host ?? "-"}</span>
            </span>
        ),
    },
    {
        id: "tok",
        header: "tokens",
        width: "56px",
        align: "right",
        cell: (s) => compact(tokenTotal(s)),
    },
    {
        id: "last",
        header: "last",
        width: "64px",
        align: "right",
        cell: (s) => <span title={iso(s.last_activity)}>{ago(s.last_activity, nowSecs())}</span>,
    },
];

export function Sessions() {
    return <WithSnapshot>{(snap) => <SessionsBody snap={snap} />}</WithSnapshot>;
}

function SessionsBody({ snap }: { snap: Snapshot }) {
    const [query, setQuery] = useState("");
    const [liveOnly, setLiveOnly] = useState(false);
    const [selectedId, setSelectedId] = useState<string>();
    const sessions = snap.sessions;
    const rows = useMemo(
        () => sessions.filter((s) => matchesSession(s, liveOnly, query)),
        [sessions, liveOnly, query],
    );
    const selected = sessions.find((s) => s.id === selectedId);
    const live = sessions.filter((s) => s.ended_at == null).length;

    if (sessions.length === 0) {
        return (
            <Panel title="sessions">
                <Empty title="No sessions yet" hint="A session appears with its first hook." />
            </Panel>
        );
    }
    return (
        <div className="flex flex-col gap-3">
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter sessions"
                        placeholder="id, host, model, project"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <div className="flex h-8 items-center gap-2 text-xs text-fg-muted">
                    <Switch checked={liveOnly} onCheckedChange={setLiveOnly} label="live only" />
                    live only
                </div>
                <Count>
                    {live} live · {sessions.length} total
                </Count>
            </Toolbar>
            <Split
                list={
                    <Panel title="sessions" hint="newest first">
                        <DataTable
                            label="sessions"
                            rows={rows}
                            columns={columns}
                            getRowId={(s) => s.id}
                            selectedId={selected?.id}
                            onSelect={(s) => setSelectedId(s.id)}
                            height={480}
                            empty={
                                <Empty
                                    title="No session matches"
                                    hint="Clear the filter or show ended sessions."
                                />
                            }
                        />
                    </Panel>
                }
                detail={selected && <Detail session={selected} snap={snap} />}
            />
        </div>
    );
}

function Detail({ session: s, snap }: { session: SessionTotals; snap: Snapshot }) {
    const calls = snap.calls.filter((c) => c.session === s.id);
    return (
        <Panel title="detail" hint={s.id.slice(0, 8)}>
            <div className="flex items-center gap-2">
                <LivePill live={s.ended_at == null} />
                <span className="text-sm font-semibold break-all">{s.id}</span>
            </div>
            <Kv
                rows={[
                    ["host", s.host ?? "-"],
                    ["project", s.project ?? "-"],
                    ["provider", s.provider ?? "-"],
                    ["api", s.api ?? "-"],
                    ["model", s.model ?? "-"],
                    [
                        "started",
                        <>
                            {hms(s.started_at)}{" "}
                            <span className="text-fg-subtle">{ago(s.started_at, nowSecs())}</span>
                        </>,
                    ],
                    ["last", hms(s.last_activity)],
                    ["ended", s.ended_at == null ? "live" : hms(s.ended_at)],
                ]}
            />
            <TokenMix tokens={s} />
            <div className="flex flex-col gap-1.5 border-t border-border/60 pt-2">
                <h3 className="text-2xs font-semibold tracking-kicker text-fg-subtle uppercase">
                    calls {calls.length}
                </h3>
                {calls.length ? (
                    <ul
                        className="max-h-64 divide-y divide-border/40 overflow-auto text-2xs"
                        tabIndex={0}
                    >
                        {calls.slice(0, 40).map((c) => (
                            <li key={c.id} className="flex items-center gap-2 py-1">
                                <span className="text-fg-muted">{hms(c.ts)}</span>
                                <SurfacePill surface={c.surface} />
                                <span className="text-fg-muted">{c.kind}</span>
                                <span className="truncate">{c.name ?? c.plugin ?? "-"}</span>
                                {!c.ok && (
                                    <span className="ml-auto">
                                        <Pill tone="fail">fail</Pill>
                                    </span>
                                )}
                            </li>
                        ))}
                    </ul>
                ) : (
                    <p className="text-2xs text-fg-muted">
                        No calls from this session in the current frame.
                    </p>
                )}
            </div>
        </Panel>
    );
}
