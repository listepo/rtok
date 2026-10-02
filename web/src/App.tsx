// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { RouterProvider } from "@tanstack/react-router";
import { DataProvider } from "./api/query";
import { connectSample, isSampleRequested } from "./api/sample";
import { connectWs } from "./api/ws";
import { createAppRouter } from "./router";

// Module-level so the providers keep one stable connection and router across re-renders.
const connect = isSampleRequested(globalThis.location.search) ? connectSample : connectWs;
const router = createAppRouter();

export default function App() {
    return (
        <DataProvider connect={connect}>
            <RouterProvider router={router} />
        </DataProvider>
    );
}
