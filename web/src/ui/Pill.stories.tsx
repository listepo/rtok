import type { Meta, StoryObj } from "@storybook/react-vite";
import { Pill } from "./Pill";

const meta = { component: Pill, args: { children: "label" } } satisfies Meta<typeof Pill>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Ok: Story = { args: { tone: "ok", dot: true, children: "pass" } };
export const Warn: Story = { args: { tone: "warn", dot: true, children: "warn" } };
export const Fail: Story = { args: { tone: "fail", dot: true, children: "fail" } };
export const Info: Story = { args: { tone: "info", children: "sample data" } };
export const Muted: Story = { args: { tone: "muted", children: "n/a" } };

// Light theme has its own fg tokens; axe checks their contrast here.
export const OkLight: Story = { ...Ok, globals: { theme: "light" } };
export const WarnLight: Story = { ...Warn, globals: { theme: "light" } };
export const FailLight: Story = { ...Fail, globals: { theme: "light" } };
export const InfoLight: Story = { ...Info, globals: { theme: "light" } };
export const MutedLight: Story = { ...Muted, globals: { theme: "light" } };
