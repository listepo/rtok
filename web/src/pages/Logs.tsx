// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useMemo, useState } from "react";
import type { Snapshot } from "../api/snapshot.gen";
import { Empty } from "../states";
import { Chip } from "../ui/Chip";
import { Panel } from "../ui/Panel";
import { Search } from "../ui/Search";
import { LEVELS, matchesLog, parseLog, type Level } from "./model";
import { Count, LevelPill, Toolbar, WithSnapshot } from "./parts";

const msgTone = { error: "text-delta-fg", warn: "text-warn-fg" } as const;

export function Logs() {
    return <WithSnapshot>{(snap) => <LogsBody snap={snap} />}</WithSnapshot>;
}

function LogsBody({ snap }: { snap: Snapshot }) {
    const [query, setQuery] = useState("");
    const [level, setLevel] = useState<Level>("all");
    const lines = useMemo(() => snap.logs.map(parseLog), [snap.logs]);
    const rows = useMemo(
        () => lines.filter((l) => matchesLog(l, level, query)),
        [lines, level, query],
    );
    const count = (lv: Level) =>
        lv === "all" ? lines.length : lines.filter((l) => l.level === lv).length;

    return (
        <div className="flex flex-col gap-3">
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search label="Filter log lines" value={query} onChange={setQuery} />
                </div>
                <div role="group" aria-label="Level" className="flex flex-wrap gap-1.5">
                    {LEVELS.map((lv) => (
                        <Chip key={lv} pressed={level === lv} onPressedChange={() => setLevel(lv)}>
                            {lv} {count(lv)}
                        </Chip>
                    ))}
                </div>
                <Count>{rows.length} lines · newest first</Count>
            </Toolbar>
            <Panel title="logs" hint="timestamps UTC, as written">
                {!lines.length ? (
                    <Empty title="No logs yet" hint="Lines appear as rtok runs." />
                ) : !rows.length ? (
                    <Empty title="No line matches" hint="Clear the filter or pick another level." />
                ) : (
                    <ol
                        aria-label="log lines"
                        tabIndex={0}
                        className="-m-4 max-h-[70vh] overflow-auto font-mono text-xs"
                    >
                        {rows.map((l) => (
                            <li
                                key={l.i}
                                className={`grid grid-cols-[2rem_minmax(0,1fr)] items-baseline gap-x-3 gap-y-0.5 border-b border-border/40 px-4 py-1.5 lg:grid-cols-[2rem_9.5rem_3.5rem_11rem_minmax(0,1fr)] ${l.level === "error" ? "bg-delta/[0.06]" : ""}`}
                            >
                                <span className="text-right text-2xs text-fg-muted">{l.i + 1}</span>
                                <span className="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 lg:contents">
                                    <span className="text-2xs text-fg-muted">{l.ts}</span>
                                    <LevelPill level={l.level} />
                                    <span className="truncate text-fg-muted">
                                        {l.source && `${l.source}/${l.name}`}
                                    </span>
                                </span>
                                <span
                                    className={`col-start-2 break-words lg:col-start-auto ${msgTone[l.level as keyof typeof msgTone] ?? ""}`}
                                >
                                    {l.msg}
                                </span>
                            </li>
                        ))}
                    </ol>
                )}
            </Panel>
        </div>
    );
}
