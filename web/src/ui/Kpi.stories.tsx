// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { Kpi } from "./Kpi";
import { Sparkline } from "./Sparkline";

const meta = {
    component: Kpi,
    args: { label: "tokens saved", value: "1.2M", sub: "62% of 1.9M" },
    decorators: [
        (Story) => (
            <div className="w-56">
                <Story />
            </div>
        ),
    ],
} satisfies Meta<typeof Kpi>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithSparkline: Story = {
    args: { viz: <Sparkline label="ctx tokens per turn" values={[3, 5, 4, 8, 6, 9, 7, 12]} /> },
};
export const Ok: Story = { args: { tone: "ok", label: "doctor", value: "all pass" } };
export const Warn: Story = { args: { tone: "warn", label: "doctor", value: "2 warn" } };
export const Fail: Story = { args: { tone: "fail", label: "doctor", value: "1 fail" } };
export const NoSub: Story = { args: { sub: undefined } };

export const WarnLight: Story = { ...Warn, globals: { theme: "light" } };
export const FailLight: Story = { ...Fail, globals: { theme: "light" } };
export const OkLight: Story = { ...Ok, globals: { theme: "light" } };
