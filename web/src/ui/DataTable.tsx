import { tableFeatures, useTable, type ColumnDef, type RowData } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef, type ReactNode } from "react";
import { Empty, Loading } from "../states";
import { focusRing } from "./cx";

export interface Column<T extends RowData> {
    id: string;
    header: string;
    /** CSS grid track, `minmax(0,1fr)` when omitted. */
    width?: string;
    align?: "right";
    cell: (row: T) => ReactNode;
}

// No sorting or filtering yet: those register as features here when a page needs them.
const features = tableFeatures({});

// ARIA table roles on divs instead of <table>: only the visible rows exist in the DOM, so
// `aria-rowcount` and `aria-rowindex` tell assistive tech how long the table really is.
export function DataTable<T extends RowData>({
    label,
    rows,
    columns,
    getRowId,
    loading = false,
    empty,
    height = 320,
    rowHeight = 36,
    selectedId,
    onSelect,
}: {
    label: string;
    rows: readonly T[];
    columns: readonly Column<T>[];
    getRowId: (row: T) => string;
    loading?: boolean;
    empty?: ReactNode;
    height?: number;
    rowHeight?: number;
    selectedId?: string;
    onSelect?: (row: T) => void;
}) {
    const defs = useMemo<ColumnDef<typeof features, T>[]>(
        () =>
            columns.map((c) => ({
                id: c.id,
                header: c.header,
                cell: ({ row }) => c.cell(row.original),
            })),
        [columns],
    );
    const table = useTable({ features, columns: defs, data: rows as T[], getRowId });
    const body = table.getRowModel().rows;
    const scroller = useRef<HTMLDivElement>(null);
    const virtualizer = useVirtualizer({
        count: body.length,
        getScrollElement: () => scroller.current,
        estimateSize: () => rowHeight,
        getItemKey: (i) => body[i]?.id ?? i,
        overscan: 8,
    });

    if (loading) return <Loading />;
    if (body.length === 0) return empty ?? <Empty title="No rows" />;

    const grid = { gridTemplateColumns: columns.map((c) => c.width ?? "minmax(0,1fr)").join(" ") };
    const align = (i: number) => (columns[i]?.align === "right" ? "text-right" : "text-left");

    return (
        <div
            role="table"
            aria-label={label}
            aria-rowcount={body.length + 1}
            className="overflow-hidden rounded-lg border border-border"
        >
            <div role="rowgroup">
                {table.getHeaderGroups().map((group) => (
                    <div
                        key={group.id}
                        role="row"
                        aria-rowindex={1}
                        style={grid}
                        className="grid h-8 items-center border-b border-border bg-surface px-3 text-2xs font-semibold tracking-table-head text-fg-subtle uppercase"
                    >
                        {group.headers.map((header, i) => (
                            <div
                                key={header.id}
                                role="columnheader"
                                className={`truncate ${align(i)}`}
                            >
                                {header.isPlaceholder ? null : <table.FlexRender header={header} />}
                            </div>
                        ))}
                    </div>
                ))}
            </div>
            {/* Rows are not tab stops unless selectable, so the scroller itself must be, or a keyboard user cannot scroll it. */}
            <div
                ref={scroller}
                role="rowgroup"
                tabIndex={onSelect ? undefined : 0}
                style={{ height, overflow: "auto" }}
                className={focusRing}
            >
                <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
                    {virtualizer.getVirtualItems().map((item) => {
                        const row = body[item.index];
                        if (!row) return null;
                        const selected = row.id === selectedId;
                        return (
                            <div
                                key={row.id}
                                role="row"
                                aria-rowindex={item.index + 2}
                                aria-selected={onSelect ? selected : undefined}
                                tabIndex={onSelect ? 0 : undefined}
                                onClick={onSelect && (() => onSelect(row.original))}
                                onKeyDown={
                                    onSelect &&
                                    ((e) => {
                                        if (e.key === "Enter" || e.key === " ") {
                                            e.preventDefault();
                                            onSelect(row.original);
                                        }
                                    })
                                }
                                style={{
                                    ...grid,
                                    position: "absolute",
                                    top: 0,
                                    width: "100%",
                                    height: item.size,
                                    transform: `translateY(${item.start}px)`,
                                }}
                                className={`${focusRing} grid items-center border-b border-border/50 px-3 text-xs hover:bg-surface-2/70 aria-selected:bg-accent/15 ${onSelect ? "cursor-pointer" : ""}`}
                            >
                                {row.getAllCells().map((cell, i) => (
                                    <div
                                        key={cell.id}
                                        role="cell"
                                        className={`truncate ${align(i)}`}
                                    >
                                        <table.FlexRender cell={cell} />
                                    </div>
                                ))}
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
