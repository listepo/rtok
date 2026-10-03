// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Decorator } from "@storybook/react-vite";
import {
    createMemoryHistory,
    createRootRoute,
    createRouter,
    RouterProvider,
} from "@tanstack/react-router";
import { useMemo, type ReactNode } from "react";
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

// The panels link to other pages, so they need a router above them even when it has one route.
function Harness({ connect, children }: { connect: Connect; children: ReactNode }) {
    const router = useMemo(
        () =>
            createRouter({
                routeTree: createRootRoute({ component: () => children }),
                history: createMemoryHistory({ initialEntries: ["/"] }),
            }),
        // The story element is fixed for the life of the harness.
        // eslint-disable-next-line react-hooks/exhaustive-deps
        [],
    );
    return (
        <DataProvider connect={connect}>
            <RouterProvider router={router} />
        </DataProvider>
    );
}

export const withData =
    (connect: Connect): Decorator =>
    (Story) => (
        <Harness connect={connect}>
            <Story />
        </Harness>
    );
