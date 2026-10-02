import type { Decorator, Meta, StoryObj } from "@storybook/react-vite";
import { DataProvider } from "../api/query";
import { connectSample } from "../api/sample";
import type { Snapshot } from "../api/snapshot.gen";
import type { Connect } from "../api/ws";
import { Calls } from "./Calls";
import { richSnapshot } from "./fixtures";
import { Overview } from "./Overview";
import { Plugins } from "./Plugins";

// One stable connection per story, as the app has; `null` never connects (loading state).
const serve =
    (snapshot: Snapshot | null): Connect =>
    (h) => {
        h.onState(snapshot ? "open" : "connecting");
        if (snapshot) h.onFrame({ type: "snapshot", snapshot });
        return { send: () => true, close: () => {} };
    };

const withData =
    (connect: Connect): Decorator =>
    (Story) => (
        <DataProvider connect={connect}>
            <Story />
        </DataProvider>
    );

const rich = serve(richSnapshot);
const loading = serve(null);
const noPlugins = serve({ ...richSnapshot, plugins: [] });
const noCalls = serve({ ...richSnapshot, calls: [] });

const overview = { title: "Pages/Overview", component: Overview } satisfies Meta<typeof Overview>;
export default overview;
type Story = StoryObj<typeof overview>;

export const OverviewDefault: Story = { decorators: [withData(rich)] };
export const OverviewLoading: Story = { decorators: [withData(loading)] };
export const OverviewNothingMeasured: Story = { decorators: [withData(noPlugins)] };
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
