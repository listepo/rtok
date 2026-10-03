// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useMemo, useState } from "react";
import type { SkillPageRow } from "../api/snapshot.gen";
import { Empty } from "../states";
import { DataTable, type Column } from "../ui/DataTable";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Search } from "../ui/Search";
import { Switch } from "../ui/Switch";
import { compact, fmt } from "./format";
import { Count, Kv, responsive, Split, Toolbar, useMinWidth, WithSnapshot } from "./parts";

// Thresholds from the design: a description past 180 chars or a body past 8 KB is what
// makes a skill expensive to keep resident.
export const LONG_DESC_CHARS = 180;
export const LONG_BODY_BYTES = 8192;

export function skillWarnings(r: SkillPageRow): string[] {
    return [
        r.desc_chars > LONG_DESC_CHARS && "long description",
        r.body_bytes > LONG_BODY_BYTES && "body > 8 KB",
        r.never && "never invoked",
    ].filter((w): w is string => w !== false);
}

export function matchesSkill(r: SkillPageRow, neverOnly: boolean, query: string): boolean {
    const q = query.trim().toLowerCase();
    return (!neverOnly || r.never) && (!q || `${r.name} ${r.source}`.toLowerCase().includes(q));
}

export function Skills() {
    return (
        <WithSnapshot>
            {(snap) => <SkillsBody header={snap.skills.header} rows={snap.skills.rows} />}
        </WithSnapshot>
    );
}

function SkillsBody({ header, rows: all }: { header: string; rows: SkillPageRow[] }) {
    const [query, setQuery] = useState("");
    const [neverOnly, setNeverOnly] = useState(false);
    const [selectedName, setSelectedName] = useState<string>();
    const rows = useMemo(
        () => all.filter((r) => matchesSkill(r, neverOnly, query)),
        [all, neverOnly, query],
    );
    const selected = all.find((r) => r.name === selectedName) ?? all[0];

    const wide = useMinWidth(768);
    const full = useMemo<Column<SkillPageRow>[]>(
        () => [
            {
                id: "skill",
                header: "skill",
                cell: (r) => (
                    <span className={r.never ? "text-fg-muted" : ""}>
                        <b>{r.name}</b> <span className="text-fg-muted">{r.source}</span>
                    </span>
                ),
            },
            {
                id: "desc",
                header: "desc",
                width: "56px",
                align: "right",
                cell: (r) => (
                    <span className={r.desc_chars > LONG_DESC_CHARS ? "text-warn-fg" : ""}>
                        {fmt(r.desc_chars)}
                    </span>
                ),
            },
            {
                id: "body",
                header: "body",
                width: "64px",
                align: "right",
                cell: (r) => (
                    <span className={r.body_bytes > LONG_BODY_BYTES ? "text-warn-fg" : ""}>
                        {compact(r.body_bytes)} B
                    </span>
                ),
            },
            {
                id: "calls",
                header: "calls",
                width: "48px",
                align: "right",
                cell: (r) => fmt(r.invocations),
            },
        ],
        [],
    );
    const columns = responsive(full, wide, ["skill", "calls"]);

    return (
        <div className="flex flex-col gap-3">
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter skills"
                        placeholder="name or source"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <label className="flex items-center gap-2 text-xs">
                    <Switch
                        checked={neverOnly}
                        onCheckedChange={setNeverOnly}
                        label="never invoked only"
                    />
                    never invoked only
                </label>
                <Count>
                    {rows.length} of {all.length}
                </Count>
            </Toolbar>
            {all.length === 0 ? (
                <Panel title="skills">
                    <Empty
                        title="No skills listed"
                        hint="The host lists no skills for this session yet."
                    />
                </Panel>
            ) : (
                <>
                    {header && (
                        <p className="glass flex flex-wrap gap-x-4 gap-y-1 px-3 py-2 text-2xs text-fg-muted">
                            {header.split(" · ").map((part) => (
                                <span key={part}>{part}</span>
                            ))}
                        </p>
                    )}
                    <Split
                        list={
                            <Panel title="skills" hint="listed by the host">
                                <DataTable
                                    label="skills"
                                    rows={rows}
                                    columns={columns}
                                    getRowId={(r) => r.name}
                                    selectedId={selected?.name}
                                    onSelect={(r) => setSelectedName(r.name)}
                                    empty={
                                        <Empty
                                            title="No skill matches"
                                            hint="Clear the filter to see every skill."
                                        />
                                    }
                                />
                            </Panel>
                        }
                        detail={selected && <Detail skill={selected} />}
                    />
                </>
            )}
        </div>
    );
}

function Detail({ skill: r }: { skill: SkillPageRow }) {
    const warnings = skillWarnings(r);
    return (
        <Panel title={r.name} hint={r.source}>
            <div className="flex flex-wrap gap-1.5">
                {warnings.length === 0 ? (
                    <Pill tone="ok">ok</Pill>
                ) : (
                    warnings.map((w) => (
                        <Pill key={w} tone={w === "never invoked" ? "muted" : "warn"}>
                            {w}
                        </Pill>
                    ))
                )}
            </div>
            <Kv
                rows={[
                    ["source", r.source],
                    [
                        "desc chars",
                        <>
                            {fmt(r.desc_chars)}{" "}
                            <span className="text-fg-subtle">
                                ≈ {fmt(Math.round(r.desc_chars / 4))} tok per request
                            </span>
                        </>,
                    ],
                    ["body bytes", fmt(r.body_bytes)],
                    ["invocations", fmt(r.invocations)],
                    ["resident", fmt(r.resident)],
                    ["last invoked", r.last_invoked],
                ]}
            />
            <p className="text-2xs text-fg-subtle">
                tokens per request use chars / 4, the estimate research.md §10.2 documents
            </p>
        </Panel>
    );
}
