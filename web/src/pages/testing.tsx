// Shared by the page tests: a fixed-snapshot connection and a mount that goes through the app's
// real router, so a page is rendered the way the shell renders it.
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { render } from "@testing-library/react";
import { DataProvider } from "../api/query";
import type { Snapshot } from "../api/snapshot.gen";
import type { Connect } from "../api/ws";
import { createAppRouter } from "../router";

export const serving =
    (snapshot: Snapshot): Connect =>
    (h) => {
        h.onState("open");
        h.onFrame({ type: "snapshot", snapshot });
        return { send: () => true, close: () => {} };
    };

export function mount(connect: Connect, path: string) {
    const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
    return render(
        <DataProvider connect={connect}>
            <RouterProvider router={router} />
        </DataProvider>,
    );
}
