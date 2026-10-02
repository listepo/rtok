// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useCallback, useMemo, useState } from "react";
import { useServerMessage, useSetMutation } from "../api/query";
import type { PluginPage } from "../api/snapshot.gen";
import { Empty } from "../states";
import { Chip } from "../ui/Chip";
import { DataTable, type Column } from "../ui/DataTable";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Search } from "../ui/Search";
import { Switch } from "../ui/Switch";
import { compact, fmt, pct } from "./format";
import { savedOf } from "./model";
import { Count, Kv, Split, Toolbar, WithSnapshot } from "./parts";

const SHOW = ["all", "on", "off", "saves"] as const;
type Show = (typeof SHOW)[number];
const SHOW_LABEL: Record<Show, string> = {
    all: "all",
    on: "enabled",
    off: "disabled",
    saves: "saves tokens",
};

export function matchesPlugin(p: PluginPage, show: Show, query: string): boolean {
    const q = query.trim().toLowerCase();
    const shown =
        show === "all" ||
        (show === "on" ? p.enabled : show === "off" ? !p.enabled : p.saves_tokens);
    return shown && (!q || `${p.id} ${p.title} ${p.summary}`.toLowerCase().includes(q));
}

export function Plugins() {
    return <WithSnapshot>{(snap) => <PluginsBody plugins={snap.plugins} />}</WithSnapshot>;
}

function PluginsBody({ plugins }: { plugins: PluginPage[] }) {
    const [query, setQuery] = useState("");
    const [show, setShow] = useState<Show>("all");
    const [selectedId, setSelectedId] = useState<string>();
    const { mutate } = useSetMutation();
    const message = useServerMessage();

    const rows = useMemo(
        () => plugins.filter((p) => matchesPlugin(p, show, query)),
        [plugins, show, query],
    );
    const selected = plugins.find((p) => p.id === selectedId) ?? plugins[0];

    // The server owns the state: the switch only asks, and the next snapshot moves it.
    const toggle = useCallback(
        (p: PluginPage) => (next: boolean) =>
            mutate({ key: `plugins.${p.id}.enabled`, value: next }),
        [mutate],
    );

    const columns = useMemo<Column<PluginPage>[]>(
        () => [
            {
                id: "on",
                header: "on",
                width: "56px",
                cell: (p) => (
                    <Switch
                        checked={p.enabled}
                        label={`toggle ${p.id}`}
                        onCheckedChange={toggle(p)}
                    />
                ),
            },
            {
                id: "plugin",
                header: "plugin",
                cell: (p) => (
                    <span className={p.enabled ? "" : "text-fg-muted"}>
                        <b>{p.title}</b>{" "}
                        <span className="text-fg-muted">{p.surfaces.join(" · ")}</span>
                    </span>
                ),
            },
            {
                id: "rows",
                header: "rows",
                width: "56px",
                align: "right",
                cell: (p) => fmt(p.stats?.rows),
            },
            {
                id: "saved",
                header: "saved",
                width: "64px",
                align: "right",
                cell: (p) => {
                    const s = savedOf(p);
                    return s != null && s > 0 ? compact(s) : "-";
                },
            },
        ],
        [toggle],
    );

    if (plugins.length === 0)
        return (
            <Panel title="plugins">
                <Empty title="No plugins in this frame" />
            </Panel>
        );

    return (
        <div className="flex flex-col gap-3">
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter plugins"
                        placeholder="id, title, summary"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <div role="group" aria-label="Show" className="flex flex-wrap gap-1.5">
                    {SHOW.map((s) => (
                        <Chip key={s} pressed={show === s} onPressedChange={() => setShow(s)}>
                            {SHOW_LABEL[s]}
                        </Chip>
                    ))}
                </div>
                <Count>{rows.length} shown</Count>
            </Toolbar>
            {message && (
                <p role="status" className="text-xs text-warn-fg">
                    {message}
                </p>
            )}
            <Split
                list={
                    <Panel title="plugins" hint="catalogue">
                        <DataTable
                            label="plugins"
                            rows={rows}
                            columns={columns}
                            getRowId={(p) => p.id}
                            selectedId={selected?.id}
                            onSelect={(p) => setSelectedId(p.id)}
                            empty={
                                <Empty
                                    title="No plugin matches"
                                    hint="Clear the filter or pick another group."
                                />
                            }
                        />
                    </Panel>
                }
                detail={selected && <Detail plugin={selected} onToggle={toggle(selected)} />}
            />
        </div>
    );
}

function Detail({
    plugin: p,
    onToggle,
}: {
    plugin: PluginPage;
    onToggle: (next: boolean) => void;
}) {
    const saved = savedOf(p);
    return (
        <Panel title={p.title} hint={p.id}>
            <div className="flex items-center gap-3">
                <Switch
                    checked={p.enabled}
                    label={`${p.enabled ? "disable" : "enable"} ${p.id}`}
                    onCheckedChange={onToggle}
                />
                <span className="text-xs">{p.enabled ? "enabled" : "disabled"}</span>
                <span className="ml-auto flex gap-1">
                    {p.surfaces.map((s) => (
                        <Pill key={s} tone="info">
                            {s}
                        </Pill>
                    ))}
                </span>
            </div>
            <p className="text-xs text-fg-muted">{p.summary}</p>
            {p.fields.length > 0 && <Kv rows={p.fields.map(([k, v]) => [String(k), String(v)])} />}
            {p.saves_tokens && p.stats ? (
                <Kv
                    rows={[
                        ["rows", fmt(p.stats.rows)],
                        [
                            "est before → after",
                            `${compact(p.stats.est_before)} → ${compact(p.stats.est_after)}`,
                        ],
                        [
                            "saved",
                            saved != null && saved > 0
                                ? `${compact(saved)} (${pct(saved / p.stats.est_before, 0)})`
                                : "-",
                        ],
                    ]}
                />
            ) : (
                <p className="text-2xs text-fg-subtle">
                    {p.saves_tokens
                        ? "no Measurement rows yet"
                        : "does not record Measurement rows"}
                </p>
            )}
            <p className="text-2xs text-fg-subtle">
                toggle sends{" "}
                <code className="text-fg-muted">{`{"set":{"key":"plugins.${p.id}.enabled"}}`}</code>
            </p>
        </Panel>
    );
}
