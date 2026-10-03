// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { Bitset, MiniBars } from "./Marks";

const meta = {
    title: "UI/Marks",
    component: MiniBars,
    args: { label: "calls per bucket", values: [3, 5, 4, 8, 6, 0, 7, 12, 10] },
} satisfies Meta<typeof MiniBars>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Bars: Story = {};
export const Coral: Story = { args: { className: "fill-delta-fg" } };
export const AllZero: Story = { args: { values: [0, 0, 0] } };
export const NoValues: Story = { args: { values: [] } };

const plugin = (id: string, enabled: boolean, saves_tokens = true) => ({
    id,
    enabled,
    saves_tokens,
});

export const PluginBitset: StoryObj = {
    render: () => (
        <Bitset
            items={[
                plugin("shell", true),
                plugin("read", true),
                plugin("graph", false),
                plugin("ledger", true, false),
                plugin("archive", true),
                plugin("budget", false),
                plugin("memory", true),
            ]}
        />
    ),
};
