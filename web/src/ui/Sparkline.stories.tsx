import type { Meta, StoryObj } from "@storybook/react-vite";
import { Sparkline } from "./Sparkline";

const meta = {
    component: Sparkline,
    args: { label: "tokens per turn", values: [3, 5, 4, 8, 6, 9, 7, 12, 10] },
} satisfies Meta<typeof Sparkline>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Area: Story = {};
export const LineOnly: Story = { args: { area: false } };
export const Flat: Story = { args: { values: [4, 4, 4, 4] } };
export const TooShort: Story = { args: { values: [4] } };
