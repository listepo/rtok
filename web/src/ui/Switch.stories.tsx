import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, userEvent, within } from "storybook/test";
import { Switch } from "./Switch";

const meta = {
    component: Switch,
    args: { label: "toggle graph", checked: false },
} satisfies Meta<typeof Switch>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Off: Story = {};
export const On: Story = { args: { checked: true } };
export const Disabled: Story = { args: { disabled: true } };
export const DisabledOn: Story = { args: { disabled: true, checked: true } };

export const Toggles: Story = {
    render: (args) => {
        const [checked, setChecked] = useState(false);
        return <Switch {...args} checked={checked} onCheckedChange={setChecked} />;
    },
    play: async ({ canvasElement }) => {
        const toggle = within(canvasElement).getByRole("switch");
        await userEvent.click(toggle);
        await expect(toggle).toHaveAttribute("aria-checked", "BROKEN");
        await userEvent.keyboard(" ");
        await expect(toggle).toHaveAttribute("aria-checked", "false");
    },
};
