// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { Panel } from "./Panel";

const meta = { component: Panel } satisfies Meta<typeof Panel>;
export default meta;
type Story = StoryObj<typeof meta>;

export const WithHint: Story = {
    args: {
        title: "Calls",
        hint: "last 200 ledger rows",
        children: <p className="text-xs">Body</p>,
    },
};

export const TitleOnly: Story = {
    args: {
        title: "Doctor",
        children: <p className="text-xs text-fg-muted">Nothing to report.</p>,
    },
};
