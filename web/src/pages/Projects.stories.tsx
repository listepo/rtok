// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { connectSample } from "../api/sample";
import { project } from "../api/sampleRows";
import type { ProjectRow } from "../api/snapshot.gen";
import { richSnapshot } from "./fixtures";
import { Graph } from "./Graph";
import { Projects } from "./Projects";
import { serve, withData } from "./storyData";

const meta = {
    title: "Pages/Graph projects",
    component: Projects,
    decorators: [withData(serve(richSnapshot))],
} satisfies Meta<typeof Projects>;
export default meta;
type Story = StoryObj<typeof meta>;

const inState = (state: string, over: Partial<ProjectRow> = {}): ProjectRow[] => [
    project(1, "rtok", { selected: true, state, ...over }),
    project(2, "ketch"),
];

export const Indexed: Story = { args: { rows: inState("ok") } };
export const NotIndexed: Story = { args: { rows: inState("not indexed", { index: null }) } };
export const Indexing: Story = { args: { rows: inState("indexing") } };
export const Stale: Story = {
    args: { rows: inState("stale", { index: { ...project(1, "x").index!, pending: 4 } }) },
};
export const Failed: Story = { args: { rows: inState("failed") } };
export const Missing: Story = {
    args: { rows: inState("missing", { missing: true, index: null }) },
};
export const NothingSelected: Story = {
    args: { rows: [project(1, "rtok"), project(2, "ketch")] },
};
export const NoProjects: Story = { args: { rows: [] } };
export const Light: Story = { args: { rows: inState("ok") }, globals: { theme: "light" } };

const many = Array.from({ length: 12 }, (_, i) =>
    project(i + 1, `project-${i + 1}`, { selected: i === 0 }),
);
export const SearchAboveTen: Story = {
    args: { rows: many },
    play: async ({ canvasElement }) => {
        const canvas = within(canvasElement);
        await userEvent.type(canvas.getByLabelText("find a project"), "project-12");
        const list = within(canvas.getByRole("list", { name: "projects" }));
        await expect(list.getAllByRole("button")).toHaveLength(1);
    },
};

// The sample server applies every request, so this is the real round trip from click to snapshot.
export const SelectAgainstSampleServer: StoryObj = {
    render: () => <Graph />,
    decorators: [withData(connectSample)],
    play: async ({ canvasElement }) => {
        const canvas = within(canvasElement);
        await userEvent.click(await canvas.findByRole("button", { name: /^ketch/ }));
        const header = within(await canvas.findByLabelText("current project"));
        await expect(await header.findByText("ketch")).toBeVisible();
        await expect(header.queryByText("rtok")).toBeNull();
    },
};
