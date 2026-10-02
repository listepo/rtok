import { useMemo, useState } from "react";
import { useExpandMutation } from "../api/query";
import type { CallRow, Snapshot } from "../api/snapshot.gen";
import { Empty } from "../states";
import { Chip } from "../ui/Chip";
import { DataTable, type Column } from "../ui/DataTable";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { Search } from "../ui/Search";
import { compact, fmt, hms, iso } from "./format";
import { tokensOf } from "./model";
import { Count, Kv, Split, SurfacePill, Toolbar, WithSnapshot } from "./parts";

const SURFACES = ["all", "hook", "mcp", "proxy"] as const;
const RESULTS = ["any", "ok", "failed"] as const;
type Surface = (typeof SURFACES)[number];
type Result = (typeof RESULTS)[number];

export function matchesCall(c: CallRow, surface: Surface, result: Result, query: string): boolean {
    const q = query.trim().toLowerCase();
    return (
        (surface === "all" || c.surface === surface) &&
        (result === "any" || (result === "ok") === Boolean(c.ok)) &&
        (!q || [c.name, c.plugin, c.session, c.kind, c.model].join(" ").toLowerCase().includes(q))
    );
}

const columns: Column<CallRow>[] = [
    {
        id: "time",
        header: "time",
        width: "72px",
        cell: (c) => <span title={iso(c.ts)}>{hms(c.ts)}</span>,
    },
    {
        id: "surface",
        header: "surface",
        width: "72px",
        cell: (c) => <SurfacePill surface={c.surface} />,
    },
    { id: "name", header: "name", cell: (c) => c.name ?? c.plugin ?? "-" },
    {
        id: "ms",
        header: "ms",
        width: "52px",
        align: "right",
        cell: (c) => (c.ms == null ? "-" : c.ms.toFixed(1)),
    },
    {
        id: "tok",
        header: "tokens",
        width: "60px",
        align: "right",
        cell: (c) => compact(tokensOf(c)),
    },
    {
        id: "ok",
        header: "ok",
        width: "52px",
        cell: (c) =>
            c.ok ? (
                <span className="text-success-fg" role="img" aria-label="ok">
                    ✓
                </span>
            ) : (
                <Pill tone="fail">fail</Pill>
            ),
    },
];

export function Calls() {
    return <WithSnapshot>{(snap) => <CallsBody snap={snap} />}</WithSnapshot>;
}

function CallsBody({ snap }: { snap: Snapshot }) {
    const [query, setQuery] = useState("");
    const [surface, setSurface] = useState<Surface>("all");
    const [result, setResult] = useState<Result>("any");
    const [selectedId, setSelectedId] = useState<number>();
    const calls = snap.calls;
    const rows = useMemo(
        () => calls.filter((c) => matchesCall(c, surface, result, query)),
        [calls, surface, result, query],
    );
    const selected = calls.find((c) => c.id === selectedId);
    const count = (s: Surface) =>
        s === "all" ? calls.length : calls.filter((c) => c.surface === s).length;

    if (calls.length === 0) {
        return (
            <Panel title="calls (newest first)">
                <Empty
                    title="No calls yet"
                    hint="The ledger fills as hooks, MCP and the proxy run."
                />
            </Panel>
        );
    }
    return (
        <div className="flex flex-col gap-3">
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter calls"
                        placeholder="name, plugin, session, model"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <div role="group" aria-label="Surface" className="flex flex-wrap gap-1.5">
                    {SURFACES.map((s) => (
                        <Chip key={s} pressed={surface === s} onPressedChange={() => setSurface(s)}>
                            {s} {count(s)}
                        </Chip>
                    ))}
                </div>
                <div role="group" aria-label="Result" className="flex flex-wrap gap-1.5">
                    {RESULTS.map((r) => (
                        <Chip key={r} pressed={result === r} onPressedChange={() => setResult(r)}>
                            {r}
                        </Chip>
                    ))}
                </div>
                <Count>
                    {rows.length} of {calls.length}
                </Count>
            </Toolbar>
            <Split
                list={
                    <Panel title="calls (newest first)" hint={`last ${calls.length} ledger rows`}>
                        <DataTable
                            label="calls"
                            rows={rows}
                            columns={columns}
                            getRowId={(c) => String(c.id)}
                            selectedId={selected && String(selected.id)}
                            onSelect={(c) => setSelectedId(c.id)}
                            height={480}
                            empty={
                                <Empty
                                    title="No call matches"
                                    hint="Clear the filter or pick another group."
                                />
                            }
                        />
                    </Panel>
                }
                detail={
                    selected && <Detail call={selected} refId={snap.ref_ids[String(selected.id)]} />
                }
            />
        </div>
    );
}

function Detail({ call: c, refId }: { call: CallRow; refId: string | undefined }) {
    const tokens = tokensOf(c);
    return (
        <Panel title="detail" hint={`#${c.id}`}>
            <div className="flex items-center gap-2">
                <SurfacePill surface={c.surface} />
                <span className="truncate text-sm font-semibold">{c.name ?? "-"}</span>
                <span className="ml-auto">
                    {c.ok ? <Pill tone="ok">ok</Pill> : <Pill tone="fail">failed</Pill>}
                </span>
            </div>
            {c.error && (
                <div
                    role="alert"
                    className="rounded-md border border-delta/40 bg-delta/10 px-2.5 py-2 text-xs text-delta-fg"
                >
                    {c.error}
                </div>
            )}
            <Kv
                rows={[
                    [
                        "time",
                        <>
                            {hms(c.ts)} <span className="text-fg-subtle">{iso(c.ts)}</span>
                        </>,
                    ],
                    ["session", c.session],
                    ["kind", c.kind],
                    ["plugin", c.plugin ?? "-"],
                    ["host", c.host ?? "-"],
                    ["provider", c.provider ?? "-"],
                    ["model", c.model ?? "-"],
                    ["api", c.api ?? "-"],
                    ["ms", c.ms == null ? "-" : c.ms.toFixed(1)],
                    [
                        "tokens",
                        tokens == null ? (
                            "-"
                        ) : (
                            <>
                                {fmt(tokens)}{" "}
                                <span className="text-fg-subtle">
                                    in {fmt(c.input)} · c+ {fmt(c.cache_create)} · cr{" "}
                                    {fmt(c.cache_read)} · out {fmt(c.output)}
                                </span>
                            </>
                        ),
                    ],
                    ["parent", c.parent_id == null ? "-" : `#${c.parent_id}`],
                    ["ref_id", refId ?? "-"],
                ]}
            />
            {refId && <Expand key={refId} refId={refId} />}
        </Panel>
    );
}

// Lossless output lives in the archive; fetching it is an explicit, per-call action.
function Expand({ refId }: { refId: string }) {
    const { mutate, data, error, isPending } = useExpandMutation();
    const [filter, setFilter] = useState("");
    const lines = (data ?? "")
        .split("\n")
        .filter((l) => !filter || l.toLowerCase().includes(filter.toLowerCase()));
    return (
        <div className="flex flex-col gap-2 border-t border-border/60 pt-2">
            <div className="flex items-center gap-2">
                <button
                    type="button"
                    disabled={isPending}
                    onClick={() => mutate(refId)}
                    className="h-8 cursor-pointer rounded-md bg-accent px-3 text-xs font-semibold text-accent-on outline-none focus-visible:shadow-ring disabled:cursor-wait disabled:opacity-60"
                >
                    expand {refId}
                </button>
                {data != null && (
                    <span className="text-2xs text-fg-subtle">{lines.length} lines</span>
                )}
            </div>
            {error && (
                <p role="alert" className="text-xs text-delta-fg">
                    {error.message}
                </p>
            )}
            {data != null && (
                <>
                    <Search label="Filter expanded output" value={filter} onChange={setFilter} />
                    <pre
                        tabIndex={0}
                        className="max-h-72 overflow-auto rounded-md border border-border bg-bg/70 p-2.5 text-2xs leading-relaxed break-words whitespace-pre-wrap text-fg-muted"
                    >
                        {lines.join("\n")}
                    </pre>
                </>
            )}
        </div>
    );
}
