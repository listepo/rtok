// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// @vitest-environment happy-dom
import { createMemoryHistory, RouterProvider } from "@tanstack/react-router";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { DataProvider } from "./api/query";
import { sampleSnapshot } from "./api/sample";
import type { Snapshot } from "./api/snapshot.gen";
import type { Connect, ConnectionState } from "./api/ws";
import { PAGES } from "./pages";
import { createAppRouter } from "./router";

// Plays the server once: pushes `state` and, when given, one snapshot.
const server =
    (state: ConnectionState, snapshot?: Snapshot): Connect =>
    (h) => {
        h.onState(state);
        if (snapshot) h.onFrame({ type: "snapshot", snapshot });
        return { send: () => true, close: () => {} };
    };

function mount(connect: Connect, path = "/overview") {
    const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
    return render(
        <DataProvider connect={connect}>
            <RouterProvider router={router} />
        </DataProvider>,
    );
}

afterEach(() => {
    cleanup();
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
});

describe("routes", () => {
    test("the route tree is the page list plus the index redirect", () => {
        const router = createAppRouter(createMemoryHistory());
        expect(Object.keys(router.routesByPath)).toEqual(["/", ...PAGES.map((p) => `/${p.id}`)]);
    });

    test("the nav lists every page as a keyboard-focusable link and each one opens its page", async () => {
        mount(server("open", sampleSnapshot));
        const nav = await screen.findByRole("navigation", { name: "Admin screens" });
        const links = Array.from(nav.querySelectorAll("a"));
        expect(links.map((a) => a.textContent)).toEqual(PAGES.map((p) => p.id));
        for (const link of links) {
            expect(link.getAttribute("href")).toMatch(/^\//);
            expect(link.tabIndex).toBeGreaterThanOrEqual(0);
            fireEvent.click(link);
            await waitFor(() => expect(link.getAttribute("aria-current")).toBe("page"));
            expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(link.textContent);
            // The heading takes focus so a keyboard user lands on the new page.
            if (link !== links[0]) {
                expect(document.activeElement).toBe(screen.getByRole("heading", { level: 1 }));
            }
        }
    });

    test("the root path redirects to the first page and unknown paths say so", async () => {
        mount(server("open", sampleSnapshot), "/");
        expect((await screen.findByRole("heading", { level: 1 })).textContent).toBe(PAGES[0].id);
        cleanup();
        mount(server("open", sampleSnapshot), "/nope");
        expect(await screen.findByText("Page not found")).toBeTruthy();
    });

    test("the skip link targets the focusable main region", async () => {
        mount(server("open", sampleSnapshot));
        const skip = await screen.findByText("Skip to content");
        expect(skip.getAttribute("href")).toBe("#main");
        expect(document.getElementById("main")?.tabIndex).toBe(-1);
    });
});

describe("theme", () => {
    test("the toggle persists the choice and a fresh mount restores it", async () => {
        localStorage.setItem("rtok-theme", "dark");
        mount(server("open", sampleSnapshot));
        fireEvent.click(await screen.findByRole("button", { name: "Switch to light theme" }));
        expect(localStorage.getItem("rtok-theme")).toBe("light");
        expect(document.documentElement.dataset.theme).toBe("light");

        cleanup();
        document.documentElement.removeAttribute("data-theme");
        mount(server("open", sampleSnapshot));
        await screen.findByRole("button", { name: "Switch to dark theme" });
        expect(document.documentElement.dataset.theme).toBe("light");
    });

    test("an unset theme follows the OS", async () => {
        mount(server("open", sampleSnapshot));
        await screen.findByRole("button", { name: /Switch to/ });
        expect(localStorage.getItem("rtok-theme")).toBeNull();
        expect(document.documentElement.dataset.theme).toMatch(/^(dark|light)$/);
    });
});

describe("shared states", () => {
    test("loading until the first snapshot", async () => {
        mount(server("connecting"));
        expect((await screen.findByRole("status")).getAttribute("aria-busy")).toBe("true");
    });

    test("offline when the socket closed", async () => {
        mount(server("closed"));
        expect(await screen.findByText("Offline")).toBeTruthy();
        expect(screen.queryByText("Loading")).toBeNull();
    });

    test("error banner carries the snapshot error", async () => {
        mount(server("open", { ...sampleSnapshot, error: "store will not open" }));
        expect((await screen.findByRole("alert")).textContent).toContain("store will not open");
    });

    test("empty when the page's data is empty", async () => {
        mount(server("open", { ...sampleSnapshot, plugins: [] }), "/plugins");
        expect(await screen.findByText("No plugins data yet")).toBeTruthy();
    });

    test("data shows no state panel", async () => {
        mount(server("open", sampleSnapshot), "/plugins");
        expect(await screen.findByText(/rows$/)).toBeTruthy();
        expect(screen.queryByRole("alert")).toBeNull();
    });
});

describe("orb", () => {
    test("falls back to the CSS gradient where WebGL is unavailable", async () => {
        mount(server("open", sampleSnapshot));
        const fallback = await screen.findByTestId("orb-fallback");
        expect(fallback.hidden).toBe(false);
    });
});
