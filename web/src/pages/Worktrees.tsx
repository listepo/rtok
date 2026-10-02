import { useMemo, useState } from "react";
import { Empty } from "../states";
import { Chip } from "../ui/Chip";
import { DataTable, type Column } from "../ui/DataTable";
import { Panel } from "../ui/Panel";
import { Pill, type PillTone } from "../ui/Pill";
import { Search } from "../ui/Search";
import {
    Count,
    Kv,
    Split,
    TextPage,
    Toolbar,
    responsive,
    useMinWidth,
    WithSnapshot,
} from "./parts";
import {
    WORKTREE_STATES,
    matchesWorktree,
    parseWorktrees,
    type WorktreeRow,
    type WorktreesView,
} from "./text";

export function Worktrees() {
    return (
        <WithSnapshot>
            {(snap) => (
                <TextPage
                    page="worktrees"
                    text={snap.worktrees}
                    command="rtok worktree list"
                    note="`rtok worktree list`; gc and clean stay CLI-only verdicts"
                >
                    {(text) => <WorktreesBody view={parseWorktrees(text)} />}
                </TextPage>
            )}
        </WithSnapshot>
    );
}

const STATE_TONE: Record<string, PillTone> = {
    main: "info",
    dirty: "warn",
    unmerged: "warn",
    merged: "ok",
    stale: "muted",
    orphan: "fail",
};

const StatePill = ({ state }: { state: string }) => (
    <Pill tone={STATE_TONE[state] ?? "muted"}>{state}</Pill>
);

/** The directory name is what tells worktrees apart; the parent path is shared noise. */
export const baseName = (path: string) => path.replace(/\/+$/, "").split("/").pop() || path;

function WorktreesBody({ view }: { view: WorktreesView }) {
    if (view.kind === "message")
        return (
            <Panel title="worktrees">
                <Empty title={view.message} />
            </Panel>
        );
    return <Table rows={view.rows} total={view.total} note={view.note} />;
}

function Table({ rows: all, total, note }: { rows: WorktreeRow[]; total: string; note: string }) {
    const [query, setQuery] = useState("");
    const [state, setState] = useState("all");
    const [selectedPath, setSelectedPath] = useState<string>();
    const rows = useMemo(
        () => all.filter((r) => matchesWorktree(r, state, query)),
        [all, state, query],
    );
    const selected = all.find((r) => r.path === selectedPath) ?? all[0];
    const count = (s: string) => all.filter((r) => s === "all" || r.state === s).length;

    const wide = useMinWidth(768);
    const full = useMemo<Column<WorktreeRow>[]>(
        () => [
            {
                id: "worktree",
                header: "worktree",
                cell: (r) => (
                    <span>
                        <b>{baseName(r.path)}</b> <span className="text-fg-muted">{r.branch}</span>
                    </span>
                ),
            },
            {
                id: "state",
                header: "state",
                width: "88px",
                cell: (r) => <StatePill state={r.state} />,
            },
            {
                id: "source",
                header: "source",
                width: "64px",
                align: "right",
                cell: (r) => r.source,
            },
            {
                id: "cache",
                header: "cache",
                width: "64px",
                align: "right",
                cell: (r) => <span className="text-fg-muted">{r.cache}</span>,
            },
        ],
        [],
    );
    const columns = responsive(full, wide, ["worktree", "state", "cache"]);

    return (
        <>
            <Toolbar>
                <div className="w-56 max-w-full">
                    <Search
                        label="Filter worktrees"
                        placeholder="path, branch, owner"
                        value={query}
                        onChange={setQuery}
                    />
                </div>
                <div role="group" aria-label="State" className="flex flex-wrap gap-1.5">
                    {["all", ...WORKTREE_STATES].map((s) => (
                        <Chip key={s} pressed={state === s} onPressedChange={() => setState(s)}>
                            {s} {count(s)}
                        </Chip>
                    ))}
                </div>
                <Count>{rows.length} shown</Count>
            </Toolbar>
            <Split
                list={
                    <Panel title="worktrees" hint={total}>
                        <DataTable
                            label="worktrees"
                            rows={rows}
                            columns={columns}
                            getRowId={(r) => r.path}
                            selectedId={selected?.path}
                            onSelect={(r) => setSelectedPath(r.path)}
                            empty={
                                <Empty
                                    title="No worktree matches"
                                    hint="Clear the filter or pick another state."
                                />
                            }
                        />
                        {note && <p className="text-2xs text-fg-subtle">{note}</p>}
                    </Panel>
                }
                detail={selected && <Detail row={selected} />}
            />
        </>
    );
}

function Detail({ row: r }: { row: WorktreeRow }) {
    return (
        <Panel title={baseName(r.path)} hint={r.state}>
            <Kv
                rows={[
                    ["path", <span className="break-all">{r.path}</span>],
                    ["branch", r.branch],
                    ["owner", r.owner],
                    ["agent", r.agent],
                    ["agent state", r.agentState],
                    ["seen", r.seen],
                    ["modified", r.modified],
                    ["source", r.source],
                    ["build cache", r.cache],
                ]}
            />
        </Panel>
    );
}
