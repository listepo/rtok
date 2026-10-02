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
