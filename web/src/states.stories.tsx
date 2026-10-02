// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { Empty, ErrorState, Loading, Offline } from "./states";

const meta = { title: "States" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

export const LoadingState: Story = { render: () => <Loading /> };
export const EmptyState: Story = {
    render: () => <Empty title="No calls yet" hint="Run an agent with rtok installed." />,
};
export const ErrorBanner: Story = { render: () => <ErrorState message="store will not open" /> };
export const OfflineBanner: Story = { render: () => <Offline /> };
