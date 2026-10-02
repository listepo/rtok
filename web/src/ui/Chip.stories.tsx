// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";
import { Chip } from "./Chip";

const meta = {
    component: Chip,
    args: { pressed: false, children: "live only" },
} satisfies Meta<typeof Chip>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Off: Story = {};
export const On: Story = { args: { pressed: true } };
export const Disabled: Story = { args: { disabled: true } };

export const Toggles: Story = {
    render: (args) => {
        const [pressed, setPressed] = useState(false);
        return <Chip {...args} pressed={pressed} onPressedChange={setPressed} />;
    },
    play: async ({ canvasElement }) => {
        const chip = within(canvasElement).getByRole("button", { name: "live only" });
        await userEvent.click(chip);
        await expect(chip).toHaveAttribute("aria-pressed", "true");
    },
};
