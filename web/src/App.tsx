// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Placeholder screen (T310.1): proves the tokens, fonts, and dark/light theme
// wiring work before the real admin screens land on top of this scaffold. T310.3 adds the
// live data layer; it only shows the connection and the plugin list until the pages arrive.
import logo from "../assets/logo.svg";
import { DataProvider, useConnection, useSnapshot } from "./api/query";
import { connectSample, isSampleRequested } from "./api/sample";
import { connectWs } from "./api/ws";

// Module-level so the provider keeps one stable connection across re-renders.
const connect = isSampleRequested(globalThis.location.search) ? connectSample : connectWs;

function Status() {
    const connection = useConnection();
    const { data } = useSnapshot();
    return (
        <>
            <p className="text-sm text-fg-muted">Connection: {connection}</p>
            {data && (
                <ul className="text-sm text-fg-muted">
                    {data.plugins.map((p) => (
                        <li key={p.id}>
                            {p.title}: {p.enabled ? "on" : "off"}
                        </li>
                    ))}
                </ul>
            )}
        </>
    );
}

export default function App() {
    return (
        <DataProvider connect={connect}>
            <main className="min-h-screen flex items-center justify-center p-6">
                <div className="glass flex flex-col items-center gap-3 px-8 py-10 text-center max-w-sm">
                    <img
                        src={logo}
                        alt=""
                        width={40}
                        height={40}
                        className="w-10 h-10 rounded-md"
                    />
                    <h1 className="text-xl font-bold tracking-[0.08em]">rtok</h1>
                    <p className="text-sm text-fg-muted">
                        The admin SPA is being built here. This screen only proves the design
                        tokens, fonts, and dark/light theme are wired up.
                    </p>
                    <Status />
                </div>
            </main>
        </DataProvider>
    );
}
