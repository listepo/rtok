// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";
import { Search } from "./Search";

const meta = {
    component: Search,
    args: {
        label: "Filter sessions",
        placeholder: "agent, project, id",
        value: "",
        onChange: () => {},
    },
} satisfies Meta<typeof Search>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};
export const Filled: Story = { args: { value: "claude" } };

export const Types: Story = {
    render: (args) => {
        const [value, setValue] = useState("");
        return <Search {...args} value={value} onChange={setValue} />;
    },
    play: async ({ canvasElement }) => {
        const input = within(canvasElement).getByRole("searchbox", { name: "Filter sessions" });
        await userEvent.type(input, "t310");
        await expect(input).toHaveValue("t310");
    },
};
