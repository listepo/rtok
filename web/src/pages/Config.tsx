import { useMemo, useState } from "react";
import { Empty } from "../states";
import { Chip } from "../ui/Chip";
import { Panel } from "../ui/Panel";
import { Pill, type PillTone } from "../ui/Pill";
import { Search } from "../ui/Search";
import { Count, TextPage, Toolbar, WithSnapshot } from "./parts";
import { CONFIG_SOURCES, groupConfig, matchesConfig, parseConfig, type ConfigEntry } from "./text";

export function Config() {
    return (
        <WithSnapshot>
            {(snap) => (
                <TextPage
                    page="config"
                    text={snap.config}
                    command="rtok config show --sources"
                    note="read-only: a page view never creates files"
                >
                    {(text) => <ConfigBody entries={parseConfig(text)} />}
                </TextPage>
            )}
        </WithSnapshot>
    );
}

// Overrides that only last as long as one process (env, flag) read as a warning: they
// are the layers that explain a value differing from the file.
export const sourceTone = (s: string): PillTone =>
    s === "default" ? "muted" : s === "env" || s === "flag" ? "warn" : "info";

function ConfigBody({ entries }: { entries: ConfigEntry[] }) {
    const [query, setQuery] = useState("");
    const [source, setSource] = useState("all");
    const rows = useMemo(
        () => entries.filter((e) => matchesConfig(e, source, query)),
        [entries, source, query],
    );
    const groups = useMemo(() => groupConfig(rows), [rows]);
    const count = (s: string) => entries.filter((e) => s === "all" || e.source === s).length;

    return (
        <>
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter config"
                        placeholder="keys and values"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <div role="group" aria-label="Source layer" className="flex flex-wrap gap-1.5">
                    {["all", ...CONFIG_SOURCES].map((s) => (
                        <Chip key={s} pressed={source === s} onPressedChange={() => setSource(s)}>
                            {s} {count(s)}
                        </Chip>
                    ))}
                </div>
                <Count>{rows.length} keys</Count>
            </Toolbar>
            <p className="text-2xs text-fg-subtle">
                precedence: default &lt; user (~/.rtok/config.toml) &lt; project (.rtok.toml) &lt;
                env (RTOK_*) &lt; flag
            </p>
            <Panel title="effective config" hint="config show --sources">
                {groups.length === 0 ? (
                    <Empty
                        title={entries.length === 0 ? "No config keys reported" : "No key matches"}
                        hint={
                            entries.length === 0
                                ? undefined
                                : "Clear the filter or pick another layer."
                        }
                    />
                ) : (
                    groups.map(([group, items]) => (
                        <section key={group} aria-label={group}>
                            <h3 className="mb-1 text-2xs font-semibold tracking-kicker text-fg-subtle uppercase">
                                [{group}]
                            </h3>
                            <dl className="divide-y divide-border/40">
                                {items.map((r) => (
                                    <div
                                        key={r.key}
                                        className="grid grid-cols-1 items-baseline gap-x-4 gap-y-0.5 py-2 text-xs sm:grid-cols-[minmax(0,16rem)_minmax(0,1fr)_auto]"
                                    >
                                        <dt className="font-semibold break-all">
                                            {r.key.slice(group.length + 1) || r.key}
                                        </dt>
                                        <dd className="break-all text-fg-muted">{r.value}</dd>
                                        <dd className="sm:text-right">
                                            <Pill tone={sourceTone(r.source)}>{r.source}</Pill>
                                        </dd>
                                    </div>
                                ))}
                            </dl>
                        </section>
                    ))
                )}
            </Panel>
        </>
    );
}
