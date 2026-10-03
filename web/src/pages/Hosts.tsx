// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { Empty, Loading } from "../states";
import { Panel } from "../ui/Panel";
import { Pill, type PillTone } from "../ui/Pill";
import { Kv, OtherLines, TextPage, WithSnapshot } from "./parts";
import { parseHosts, type HostBlock, type HostsView } from "./text";

export function Hosts() {
    return (
        <WithSnapshot>
            {(snap) => (
                <TextPage
                    page="hosts"
                    text={snap.hosts}
                    command="rtok agents list"
                    note="`rtok agents list`, one block per host variant, cached so the 2 s tick never waits"
                >
                    {(text) => <HostsBody view={parseHosts(text)} />}
                </TextPage>
            )}
        </WithSnapshot>
    );
}

/** The state word a module row starts with; the rest (a flag to run) stays beside it. */
export function moduleState(value: string): { tone: PillTone; label: string; rest: string } {
    for (const [prefix, tone] of [
        ["installed", "ok"],
        ["not installed", "warn"],
        ["not supported", "muted"],
    ] as const) {
        if (value.startsWith(prefix))
            return { tone, label: prefix, rest: value.slice(prefix.length).trim() };
    }
    return { tone: "muted", label: value, rest: "" };
}

export function hostNote(note: string): { tone: PillTone; label: string } {
    if (note === "not found") return { tone: "muted", label: "not found" };
    if (note === "not installed") return { tone: "warn", label: "not installed" };
    return { tone: "ok", label: note || "present" };
}

function HostsBody({ view }: { view: HostsView }) {
    if (view.probing)
        return (
            <Panel title="hosts">
                <Loading />
                <p className="text-2xs text-fg-subtle">
                    probing hosts: the first probe runs in the background and the tick never blocks
                    on it
                </p>
            </Panel>
        );
    if (view.blocks.length === 0 && view.other.length === 0)
        return (
            <Panel title="hosts">
                <Empty title="No hosts reported" />
            </Panel>
        );
    return (
        <>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
                {view.blocks.map((b) => (
                    <Host key={`${b.kind}-${b.name}`} block={b} />
                ))}
            </div>
            <OtherLines lines={view.other} />
        </>
    );
}

function Host({ block: b }: { block: HostBlock }) {
    const note = hostNote(b.note);
    return (
        <Panel title={b.name} hint={b.kind}>
            <Pill tone={note.tone} dot={note.tone === "ok"}>
                {note.label}
            </Pill>
            <Kv
                rows={[
                    ["app", b.app ?? "-"],
                    [
                        "config",
                        b.config.length === 0
                            ? "-"
                            : b.config.map((c) => (
                                  <span key={c} className="block break-all">
                                      {c}
                                  </span>
                              )),
                    ],
                ]}
            />
            {b.modules.length > 0 && (
                <ul className="flex flex-col gap-1.5 border-t border-border/60 pt-3">
                    {b.modules.map(([name, value]) => {
                        const s = moduleState(value);
                        return (
                            <li key={name} className="flex items-center gap-2 text-xs">
                                <span className="w-14 shrink-0 text-fg-muted">{name}</span>
                                <Pill tone={s.tone} dot={s.tone === "ok"}>
                                    {s.label}
                                </Pill>
                                {s.rest && (
                                    <code className="text-2xs text-fg-subtle">{s.rest}</code>
                                )}
                            </li>
                        );
                    })}
                </ul>
            )}
            {b.skip && (
                <p className="border-t border-border/60 pt-3 text-2xs text-fg-muted">{b.skip}</p>
            )}
        </Panel>
    );
}
