// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useState } from "react";
import { useProjectMutation, useServerMessage } from "../api/query";
import type { ProjectRow } from "../api/snapshot.gen";
import { Empty } from "../states";
import { focusRing } from "../ui/cx";
import { Kpi } from "../ui/Kpi";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Search } from "../ui/Search";
import { fmt } from "./format";
import { filterProjects, linkTargets, SEARCH_ABOVE, stateOf } from "./projectLogic";

/** The server owns the registry: every control only asks, and the next snapshot moves the page. */
export function Projects({ rows }: { rows: ProjectRow[] }) {
    const current = rows.find((p) => p.selected);
    return (
        <Panel title="projects" hint={`${rows.length} registered`}>
            {rows.length === 0 ? (
                <Empty title="No projects" hint="Run `rtok graph projects add <path>`." />
            ) : (
                <>
                    <Selector rows={rows} />
                    {current ? (
                        <Current rows={rows} p={current} />
                    ) : (
                        <p className="text-xs text-fg-muted">No project selected.</p>
                    )}
                </>
            )}
        </Panel>
    );
}

const button = `${focusRing} h-7 cursor-pointer rounded-md border border-border px-2.5 text-2xs font-semibold hover:border-border-strong disabled:cursor-not-allowed disabled:opacity-40`;

function Selector({ rows }: { rows: ProjectRow[] }) {
    const [query, setQuery] = useState("");
    const { mutate } = useProjectMutation();
    const shown = filterProjects(rows, query);
    const message = useServerMessage();
    return (
        <div className="flex flex-col gap-2">
            {rows.length > SEARCH_ABOVE && (
                <Search label="find a project" value={query} onChange={setQuery} />
            )}
            <ul aria-label="projects" className="flex max-h-56 flex-col gap-1 overflow-y-auto">
                {shown.map((p) => (
                    <li key={p.id}>
                        <button
                            type="button"
                            aria-pressed={p.selected}
                            disabled={p.missing}
                            onClick={() => mutate({ action: "select", project: String(p.id) })}
                            className={`${focusRing} flex w-full items-center gap-2 rounded-md border border-border px-2.5 py-1.5 text-left text-xs hover:border-border-strong aria-pressed:border-accent disabled:cursor-not-allowed disabled:opacity-50`}
                        >
                            <b className="truncate">{p.name}</b>
                            <Pill tone={stateOf(p).tone}>{stateOf(p).label}</Pill>
                            <span className="ml-auto text-2xs text-fg-muted">{p.origin}</span>
                        </button>
                    </li>
                ))}
            </ul>
            {shown.length === 0 && <Empty title="No project matches" />}
            {message && <p className="text-2xs text-warn-fg">{message}</p>}
        </div>
    );
}

function Current({ rows, p }: { rows: ProjectRow[]; p: ProjectRow }) {
    const s = stateOf(p);
    return (
        <>
            <div aria-label="current project" className="flex flex-col gap-2">
                <p className="flex flex-wrap items-center gap-2 text-sm">
                    <b>{p.name}</b>
                    <Pill tone={s.tone} dot>
                        {s.label}
                    </Pill>
                    <span className="text-2xs text-fg-muted">{s.hint}</span>
                </p>
                <p className="truncate text-2xs text-fg-subtle">{p.root}</p>
                {p.index && (
                    <div className="grid grid-cols-3 gap-2">
                        <Kpi label="rows" value={fmt(p.index.rows)} />
                        <Kpi label="files" value={fmt(p.index.files)} />
                        <Kpi
                            label="pending"
                            value={fmt(p.index.pending)}
                            tone={p.index.pending ? "warn" : "ok"}
                        />
                    </div>
                )}
            </div>
            <Links rows={rows} p={p} />
        </>
    );
}

function Links({ rows, p }: { rows: ProjectRow[]; p: ProjectRow }) {
    const { mutate } = useProjectMutation();
    const targets = linkTargets(rows, p);
    const [to, setTo] = useState("");
    const [both, setBoth] = useState(false);
    const from = String(p.id);
    const target = targets.some((t) => String(t.id) === to) ? to : "";
    return (
        <section aria-label="links" className="flex flex-col gap-2">
            <h3 className="text-2xs font-semibold text-fg-muted">linked projects</h3>
            {p.links.length === 0 ? (
                <p className="text-xs text-fg-muted">{p.name} is not linked to another project.</p>
            ) : (
                <ul className="flex flex-col divide-y divide-border/60 text-xs">
                    {p.links.map((l) => (
                        <li key={l.to} className="flex items-center gap-2 py-1.5">
                            <b className="truncate">{l.name}</b>
                            <Pill tone={l.kind === "manual" ? "info" : "muted"}>{l.kind}</Pill>
                            {l.reason && <span className="truncate text-fg-muted">{l.reason}</span>}
                            <button
                                type="button"
                                className={`${button} ml-auto`}
                                aria-label={`unlink ${l.name}`}
                                onClick={() =>
                                    mutate({
                                        action: "unlink",
                                        from,
                                        to: String(l.to),
                                        both: false,
                                    })
                                }
                            >
                                unlink
                            </button>
                        </li>
                    ))}
                </ul>
            )}
            <div className="flex flex-wrap items-center gap-2 text-xs">
                <select
                    aria-label="link to"
                    value={target}
                    onChange={(e) => setTo(e.target.value)}
                    className={`${focusRing} h-7 rounded-md border border-border bg-bg px-2`}
                >
                    <option value="">link to…</option>
                    {targets.map((t) => (
                        <option key={t.id} value={t.id}>
                            {t.name}
                        </option>
                    ))}
                </select>
                <label className="flex items-center gap-1.5 text-fg-muted">
                    <input
                        type="checkbox"
                        checked={both}
                        onChange={(e) => setBoth(e.target.checked)}
                    />
                    both ways
                </label>
                <button
                    type="button"
                    className={button}
                    disabled={!target}
                    onClick={() => {
                        mutate({ action: "link", from, to: target, both });
                        setTo("");
                    }}
                >
                    link
                </button>
            </div>
        </section>
    );
}
