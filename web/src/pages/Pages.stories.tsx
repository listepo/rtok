import type { Meta, StoryObj } from "@storybook/react-vite";
import { connectSample } from "../api/sample";
import { Calls } from "./Calls";
import { richSnapshot } from "./fixtures";
import { Overview } from "./Overview";
import { Plugins } from "./Plugins";
import { serve, withData } from "./storyData";

const rich = serve(richSnapshot);
const loading = serve(null);
const noPlugins = serve({ ...richSnapshot, plugins: [] });
const noCalls = serve({ ...richSnapshot, calls: [] });
const noDoctor = serve({ ...richSnapshot, doctor: null, sessions: [] });

const overview = { title: "Pages/Overview", component: Overview } satisfies Meta<typeof Overview>;
export default overview;
type Story = StoryObj<typeof overview>;

export const OverviewDefault: Story = { decorators: [withData(rich)] };
export const OverviewLoading: Story = { decorators: [withData(loading)] };
export const OverviewNothingMeasured: Story = { decorators: [withData(noPlugins)] };
export const OverviewDoctorUnavailable: Story = { decorators: [withData(noDoctor)] };
export const OverviewLight: Story = { decorators: [withData(rich)], globals: { theme: "light" } };

export const PluginsDefault: StoryObj = { render: () => <Plugins />, decorators: [withData(rich)] };
export const PluginsLight: StoryObj = {
    render: () => <Plugins />,
    decorators: [withData(rich)],
    globals: { theme: "light" },
};
export const PluginsEmpty: StoryObj = {
    render: () => <Plugins />,
    decorators: [withData(noPlugins)],
};
// The sample server applies the toggle, so this is the real round-trip.
export const PluginsToggleAgainstSampleServer: StoryObj = {
    render: () => <Plugins />,
    decorators: [withData(connectSample)],
};

export const CallsDefault: StoryObj = { render: () => <Calls />, decorators: [withData(rich)] };
export const CallsLight: StoryObj = {
    render: () => <Calls />,
    decorators: [withData(rich)],
    globals: { theme: "light" },
};
export const CallsEmpty: StoryObj = { render: () => <Calls />, decorators: [withData(noCalls)] };
