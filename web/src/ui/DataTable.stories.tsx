import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";
import { sampleSnapshot } from "../api/sample";
import type { CallRow } from "../api/snapshot.gen";
import { Empty, ErrorState } from "../states";
import { DataTable, type Column } from "./DataTable";
import { Pill } from "./Pill";

const columns: Column<CallRow>[] = [
    { id: "surface", header: "surface", width: "90px", cell: (r) => r.surface },
    { id: "kind", header: "kind", width: "90px", cell: (r) => r.kind },
    { id: "name", header: "name", cell: (r) => r.name ?? "-" },
    { id: "ms", header: "ms", width: "70px", align: "right", cell: (r) => r.ms ?? "-" },
    {
        id: "ok",
        header: "ok",
        width: "80px",
        cell: (r) => (r.ok ? <Pill tone="ok">ok</Pill> : <Pill tone="fail">error</Pill>),
    },
];

const many = Array.from({ length: 10_000 }, (_, i): CallRow => {
    const base = sampleSnapshot.calls[i % sampleSnapshot.calls.length] as CallRow;
    return { ...base, id: i };
});

const meta = {
    component: DataTable<CallRow>,
    args: {
        label: "sample calls",
        columns,
        rows: sampleSnapshot.calls,
        getRowId: (r: CallRow) => String(r.id),
    },
} satisfies Meta<typeof DataTable<CallRow>>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Loading: Story = { args: { loading: true } };
export const EmptyDefault: Story = { args: { rows: [] } };
export const EmptyCustom: Story = {
    args: {
        rows: [],
        empty: <Empty title="No calls yet" hint="Run an agent with rtok installed." />,
    },
};
export const Failed: Story = {
    args: { rows: [], empty: <ErrorState message="store will not open" /> },
};
export const TenThousandRows: Story = {
    args: { rows: many },
    play: async ({ canvasElement }) => {
        const table = within(canvasElement).getByRole("table");
        await expect(table).toHaveAttribute("aria-rowcount", "10001");
        // Windowed: far fewer rows in the DOM than in the data.
        await expect(within(table).getAllByRole("row").length).toBeLessThan(60);
    },
};

export const Selectable: Story = {
    render: (args) => {
        const [selected, setSelected] = useState<string>();
        return (
            <DataTable
                {...args}
                selectedId={selected}
                onSelect={(r) => setSelected(String(r.id))}
            />
        );
    },
    play: async ({ canvasElement }) => {
        const rows = within(canvasElement).getAllByRole("row");
        const first = rows[1] as HTMLElement;
        first.focus();
        await userEvent.keyboard("{Enter}");
        await expect(first).toHaveAttribute("aria-selected", "true");
    },
};
