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
