import type { Decorator } from "@storybook/react-vite";
import { DataProvider } from "../api/query";
import type { Snapshot } from "../api/snapshot.gen";
import type { Connect } from "../api/ws";

// One stable connection per story, as the app has; `null` never connects (loading state).
export const serve =
    (snapshot: Snapshot | null): Connect =>
    (h) => {
        h.onState(snapshot ? "open" : "connecting");
        if (snapshot) h.onFrame({ type: "snapshot", snapshot });
        return { send: () => true, close: () => {} };
    };

export const withData =
    (connect: Connect): Decorator =>
    (Story) => (
        <DataProvider connect={connect}>
            <Story />
        </DataProvider>
    );
