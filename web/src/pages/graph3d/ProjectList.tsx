// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { Empty } from "../../states";
import { focusRing } from "../../ui/cx";
import { Pill } from "../../ui/Pill";
import { stateOf } from "../projectLogic";
import type { Scene } from "./scene";

/**
 * The overview as a plain list: the keyboard and screen-reader view of what the canvas draws,
 * available beside 3D and 2D. Each project names its outgoing links and whether it is in scope.
 */
export function ProjectList({ scene, select }: { scene: Scene; select(id: number): void }) {
    if (scene.nodes.length === 0) return <Empty title="No project matches" />;
    const name = new Map(scene.nodes.map((n) => [n.id, n.label]));
    return (
        <ul aria-label="project graph" className="flex flex-col gap-1">
            {scene.nodes.map((n) => {
                const out = scene.edges.filter((e) => e.from === n.id);
                const s = stateOf(n);
                return (
                    <li key={n.id}>
                        <button
                            type="button"
                            aria-pressed={n.selected}
                            disabled={n.hollow}
                            onClick={() => select(n.id)}
                            className={`${focusRing} flex w-full flex-wrap items-center gap-2 rounded-md border border-border px-2.5 py-1.5 text-left text-xs hover:border-border-strong aria-pressed:border-accent disabled:cursor-not-allowed disabled:opacity-50`}
                        >
                            <span
                                aria-hidden
                                className="size-2.5 shrink-0 rounded-full"
                                style={{ background: n.color }}
                            />
                            <b className="truncate">{n.label}</b>
                            <Pill tone={s.tone}>{s.label}</Pill>
                            {n.inScope && !n.selected && <Pill tone="info">in scope</Pill>}
                            <span className="ml-auto text-2xs text-fg-muted">{n.origin}</span>
                            {out.length > 0 && (
                                <span className="w-full truncate text-2xs text-fg-subtle">
                                    links to{" "}
                                    {out.map((e) => `${name.get(e.to)} (${e.kind})`).join(", ")}
                                </span>
                            )}
                        </button>
                    </li>
                );
            })}
        </ul>
    );
}
