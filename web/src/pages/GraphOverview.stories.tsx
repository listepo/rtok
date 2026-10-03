// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Decorator, Meta, StoryObj } from "@storybook/react-vite";
import { createRef, type ReactNode, useEffect, useRef } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import { connectSample } from "../api/sample";
import { project } from "../api/sampleRows";
import type { ProjectRow } from "../api/snapshot.gen";
import { richSnapshot } from "./fixtures";
import { VIEW_KEY, ProjectsOverview } from "./graph3d/ProjectsOverview";
import type { ViewApi } from "./graph3d/webgl";
import { Projects } from "./Projects";
import { WithSnapshot } from "./parts";
import { serve, withData } from "./storyData";

const meta = {
    title: "Pages/Graph overview",
    component: ProjectsOverview,
    decorators: [withData(serve(richSnapshot))],
} satisfies Meta<typeof ProjectsOverview>;
export default meta;
type Story = StoryObj<typeof meta>;

const registry: ProjectRow[] = [
    project(1, "rtok", {
        selected: true,
        links: [{ to: 2, kind: "manual", name: "ketch", reason: null }],
    }),
    project(2, "ketch", {
        links: [{ to: 3, kind: "auto", name: "pyrlyn", reason: "shared deps" }],
    }),
    project(3, "pyrlyn", { state: "stale" }),
    project(4, "landing"),
    project(5, "gone", { state: "missing", missing: true, index: null }),
];

/** The stories pick the view through the same key a reload reads. */
const viewing =
    (view: string): Decorator =>
    (Story) => {
        try {
            localStorage.setItem(VIEW_KEY, view);
        } catch {
            // Storage blocked: the default (3D) applies.
        }
        return <Story />;
    };

/** WebGL off: what a browser without a GPU or with the feature disabled reports. Restored on unmount so later stories keep their GPU. */
function WithoutWebgl({ children }: { children: ReactNode }) {
    const proto = HTMLCanvasElement.prototype;
    const real = useRef(proto.getContext);
    if (proto.getContext === real.current) {
        proto.getContext = function (this: HTMLCanvasElement, kind: string, ...rest: unknown[]) {
            return kind.startsWith("webgl")
                ? null
                : (real.current as Function).call(this, kind, ...rest);
        } as typeof real.current;
    }
    useEffect(() => () => void (proto.getContext = real.current), [proto]);
    return <>{children}</>;
}
const noWebgl: Decorator = (Story) => (
    <WithoutWebgl>
        <Story />
    </WithoutWebgl>
);

/** Pixels the stage drew: the canvas is transparent where nothing is. */
function drawn(canvas: HTMLCanvasElement): number {
    const copy = document.createElement("canvas");
    copy.width = canvas.width;
    copy.height = canvas.height;
    const ctx = copy.getContext("2d")!;
    ctx.drawImage(canvas, 0, 0);
    const px = ctx.getImageData(0, 0, copy.width, copy.height).data;
    let n = 0;
    for (let i = 3; i < px.length; i += 4) if (px[i]! > 0) n++;
    return n;
}

export const List: Story = { args: { rows: registry }, decorators: [viewing("list")] };
export const TwoD: Story = { args: { rows: registry }, decorators: [viewing("2d")] };
export const Light: Story = {
    args: { rows: registry },
    decorators: [viewing("2d")],
    globals: { theme: "light" },
};
export const Single: Story = {
    args: { rows: [project(1, "solo", { selected: true })] },
    decorators: [viewing("2d")],
};

const many = Array.from({ length: 60 }, (_, i) =>
    project(i + 1, `project-${i + 1}`, {
        origin: (["manual", "session", "worktree", "mcp"] as const)[i % 4]!,
        links: i
            ? [{ to: ((i * 7) % i) + 1, kind: i % 3 ? "auto" : "manual", name: "x", reason: null }]
            : [],
    }),
);
export const Clustered: Story = { args: { rows: many }, decorators: [viewing("2d")] };

export const WebglOffShowsTwoD: Story = {
    args: { rows: registry },
    decorators: [noWebgl, viewing("3d")],
    play: async ({ canvasElement }) => {
        const canvas = within(canvasElement);
        await expect((await canvas.findByRole("status")).textContent).toMatch(
            /3D view unavailable/,
        );
        await waitFor(() => expect(canvas.getAllByTestId("node-2d")).toHaveLength(5));
        await expect(canvas.queryByTestId("graph-3d")).toBeNull();
    },
};

export const WebglDrawsAndClickSelects: StoryObj = {
    render: () => {
        const probe = createRef<ViewApi | null>() as { current: ViewApi | null };
        (globalThis as { __probe?: typeof probe }).__probe = probe;
        return (
            <WithSnapshot>
                {(snap) => (
                    <>
                        <Projects rows={snap.projects ?? []} />
                        <ProjectsOverview rows={snap.projects ?? []} probe={probe} />
                    </>
                )}
            </WithSnapshot>
        );
    },
    decorators: [withData(connectSample), viewing("3d")],
    play: async ({ canvasElement }) => {
        const canvas = within(canvasElement);
        const host = await canvas.findByTestId("graph-3d");
        const gl = await waitFor(() => {
            const c = host.querySelector("canvas");
            if (!c) throw new Error("no canvas yet");
            return c;
        });
        await waitFor(() => expect(drawn(gl)).toBeGreaterThan(200), { timeout: 8000 });

        const probe = (globalThis as { __probe?: { current: ViewApi | null } }).__probe!;
        // ketch is not selected in the sample registry; a click on its sphere selects it.
        const target = await waitFor(() => {
            const at = probe.current?.screenOf(2);
            if (!at) throw new Error("layout not ready");
            return at;
        });
        const coords = { clientX: target.x, clientY: target.y };
        await userEvent.pointer([
            { keys: "[MouseLeft>]", target: gl, coords },
            { keys: "[/MouseLeft]", target: gl, coords },
        ]);
        const header = within(await canvas.findByLabelText("current project"));
        await expect(await header.findByText("ketch")).toBeVisible();
    },
};
