// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { lazy, type RefObject, Suspense, useEffect, useRef, useState } from "react";
import { useProjectMutation } from "../../api/query";
import type { ProjectRow } from "../../api/snapshot.gen";
import { Empty } from "../../states";
import { focusRing } from "../../ui/cx";
import { Kpi } from "../../ui/Kpi";
import { Panel } from "../../ui/Panel";
import { Search } from "../../ui/Search";
import { Switch } from "../../ui/Switch";
import { ProjectList } from "./ProjectList";
import Scene2D from "./Scene2D";
import { buildScene, type Scene, signature } from "./scene";
import { useLayout } from "./useLayout";
import { type Hover, prefersReducedMotion, type ViewApi, webglAvailable } from "./webgl";

// Three.js is a separate chunk: a page that never draws in 3D never downloads it.
const Scene3D = lazy(() => import("./Scene3D"));

export type View = "3d" | "2d" | "list";
export const VIEW_KEY = "rtok.graph.view";
const VIEWS: { id: View; label: string }[] = [
    { id: "3d", label: "3D" },
    { id: "2d", label: "2D" },
    { id: "list", label: "List" },
];

/** The 2D/3D/list choice survives a reload; storage can be blocked, and then it simply is not kept. */
export function readView(): View {
    try {
        const v = localStorage.getItem(VIEW_KEY);
        if (v === "3d" || v === "2d" || v === "list") return v;
    } catch {
        // Storage is blocked: fall through to the default.
    }
    return "3d";
}

function saveView(v: View) {
    try {
        localStorage.setItem(VIEW_KEY, v);
    } catch {
        // Not kept; the choice still applies until the page is closed.
    }
}

/** A new scene object only when what is drawn changed, so a snapshot that moves nothing rebuilds nothing. */
function useScene(rows: ProjectRow[], query: string, scopeOnly: boolean): Scene {
    const next = buildScene(rows, { query, scopeOnly });
    const kept = useRef(next);
    if (signature(kept.current) !== signature(next)) kept.current = next;
    return kept.current;
}

/** Level 1 of the graph page (T329 §8a): the registered projects and their links. */
export function ProjectsOverview({
    rows,
    probe,
}: {
    rows: ProjectRow[];
    /** Hands the active view's API to a test, which has no other way to learn where a node is on screen. */
    probe?: RefObject<ViewApi | null>;
}) {
    const [query, setQuery] = useState("");
    const [scopeOnly, setScopeOnly] = useState(false);
    const [choice, setChoice] = useState<View>(readView);
    const [canGl] = useState(webglAvailable);
    const [lost, setLost] = useState<string | null>(null);
    const [tip, setTip] = useState<Hover | null>(null);
    const [menu, setMenu] = useState<{ id: number; x: number; y: number } | null>(null);
    const scene = useScene(rows, query, scopeOnly);
    const positions = useLayout(scene);
    const own = useRef<ViewApi | null>(null);
    const api = probe ?? own;
    const { mutate } = useProjectMutation();

    const view: View = choice === "3d" && (!canGl || lost !== null) ? "2d" : choice;
    const why = lost ?? (canGl ? null : "WebGL is not available in this browser");
    const pick = (v: View) => {
        setChoice(v);
        saveView(v);
        setMenu(null);
        setTip(null);
    };
    const select = (id: number) => {
        const p = rows.find((r) => r.id === id);
        if (p && !p.missing) mutate({ action: "select", project: String(id) });
    };
    const events = {
        select,
        menu: (id: number, x: number, y: number) => setMenu({ id, x, y }),
        hover: setTip,
    };

    return (
        <Panel title="project graph" hint={`${scene.counts.total} projects`}>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
                <Kpi label="projects" value={String(scene.counts.total)} />
                <Kpi label="in scope" value={String(scene.counts.inScope)} />
                <Kpi label="linked pairs" value={String(scene.counts.pairs)} />
                <Kpi
                    label="problems"
                    value={String(scene.counts.problems)}
                    tone={scene.counts.problems ? "warn" : "ok"}
                    sub="missing or failed"
                />
            </div>
            <div className="flex flex-wrap items-end gap-3">
                <div className="w-48">
                    <Search label="filter projects" value={query} onChange={setQuery} />
                </div>
                <label className="flex items-center gap-1.5 text-xs text-fg-muted">
                    <Switch
                        label="only the current scope"
                        checked={scopeOnly}
                        onCheckedChange={setScopeOnly}
                    />
                    only scope
                </label>
                <div role="group" aria-label="view" className="ml-auto flex gap-1">
                    {VIEWS.map((v) => (
                        <button
                            key={v.id}
                            type="button"
                            aria-pressed={choice === v.id}
                            onClick={() => pick(v.id)}
                            className={`${focusRing} rounded-md border border-border px-2.5 py-1 text-xs hover:border-border-strong aria-pressed:border-accent`}
                        >
                            {v.label}
                        </button>
                    ))}
                </div>
                {view !== "list" && (
                    <div className="flex gap-1">
                        {(["fit", "reset"] as const).map((a) => (
                            <button
                                key={a}
                                type="button"
                                onClick={() => api.current?.[a]()}
                                className={`${focusRing} rounded-md border border-border px-2.5 py-1 text-xs hover:border-border-strong`}
                            >
                                {a === "fit" ? "Fit all" : "Reset view"}
                            </button>
                        ))}
                    </div>
                )}
            </div>
            {choice === "3d" && why && (
                <p
                    role="status"
                    className="rounded-md border border-warn/40 px-2.5 py-1.5 text-xs text-warn-fg"
                >
                    3D view unavailable ({why}); showing the 2D view.
                </p>
            )}
            {rows.length === 1 && (
                <p className="text-2xs text-fg-subtle">
                    One project so far. Link another with `rtok graph projects link &lt;project&gt;`
                    to see edges.
                </p>
            )}
            {view === "list" ? (
                <ProjectList scene={scene} select={select} />
            ) : scene.nodes.length === 0 ? (
                <Empty title="No project matches" />
            ) : (
                <div className="relative h-[28rem] overflow-hidden rounded-md border border-border">
                    {view === "3d" ? (
                        <Suspense
                            fallback={
                                <p className="p-3 text-xs text-fg-muted">Loading the 3D view…</p>
                            }
                        >
                            <Scene3D
                                scene={scene}
                                positions={positions}
                                api={api}
                                {...events}
                                onUnavailable={setLost}
                            />
                        </Suspense>
                    ) : (
                        <Scene2D scene={scene} positions={positions} api={api} {...events} />
                    )}
                </div>
            )}
            {tip && (
                <div
                    role="tooltip"
                    style={{ left: tip.x + 12, top: tip.y + 12 }}
                    className="pointer-events-none fixed z-30 rounded-md border border-border-strong bg-surface-2 px-2 py-1 text-2xs text-fg shadow-lg"
                >
                    {tip.text}
                </div>
            )}
            {menu && (
                <NodeMenu
                    at={menu}
                    node={rows.find((r) => r.id === menu.id)}
                    onClose={() => setMenu(null)}
                    onSelect={() => select(menu.id)}
                    onFocus={() => api.current?.focus(menu.id)}
                />
            )}
        </Panel>
    );
}

/** What the protocol offers today: select, focus, copy the root. Link, unlink, re-index and remove arrive with T329.20. */
function NodeMenu({
    at,
    node,
    onClose,
    onSelect,
    onFocus,
}: {
    at: { x: number; y: number };
    node: ProjectRow | undefined;
    onClose(): void;
    onSelect(): void;
    onFocus(): void;
}) {
    const ref = useRef<HTMLDivElement>(null);
    useEffect(() => {
        ref.current?.querySelector("button")?.focus();
        const away = (e: Event) => !ref.current?.contains(e.target as Node) && onClose();
        const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
        document.addEventListener("pointerdown", away);
        document.addEventListener("keydown", key);
        return () => {
            document.removeEventListener("pointerdown", away);
            document.removeEventListener("keydown", key);
        };
    }, [onClose]);
    if (!node) return null;
    const item =
        "block w-full px-3 py-1.5 text-left text-xs hover:bg-surface-3 disabled:opacity-40";
    const act = (fn: () => void) => () => (fn(), onClose());
    return (
        <div
            ref={ref}
            role="menu"
            aria-label={`${node.name} actions`}
            style={{ left: at.x, top: at.y }}
            className="fixed z-40 min-w-36 rounded-md border border-border-strong bg-surface-2 py-1 shadow-lg"
        >
            <button
                type="button"
                role="menuitem"
                disabled={node.missing}
                className={item}
                onClick={act(onSelect)}
            >
                Select
            </button>
            <button type="button" role="menuitem" className={item} onClick={act(onFocus)}>
                {prefersReducedMotion() ? "Center" : "Fly to"}
            </button>
            <button
                type="button"
                role="menuitem"
                className={item}
                onClick={act(() => void navigator.clipboard?.writeText(node.root).catch(() => {}))}
            >
                Copy path
            </button>
        </div>
    );
}
