// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import {
    createHashHistory,
    createRootRoute,
    createRoute,
    createRouter,
    redirect,
    type RouteComponent,
    type RouterHistory,
} from "@tanstack/react-router";
import { useConnection, useSnapshot } from "./api/query";
import { Calls } from "./pages/Calls";
import { Config } from "./pages/Config";
import { Graph } from "./pages/Graph";
import { Hosts } from "./pages/Hosts";
import { Overview } from "./pages/Overview";
import { Plugins } from "./pages/Plugins";
import { Services } from "./pages/Services";
import { Skills } from "./pages/Skills";
import { Stats } from "./pages/Stats";
import { Worktrees } from "./pages/Worktrees";
import { PAGES, type Page } from "./pages";
import { NotFound, Shell } from "./Shell";
import { Empty, Loading } from "./states";

// Real pages replace this per id in T310.6+; until then it only proves each route reads
// its snapshot field and shows the shared states.
function PagePlaceholder({ page }: { page: Page }) {
    const connection = useConnection();
    const { data } = useSnapshot();
    if (!data) return connection === "closed" ? null : <Loading />;
    const value = data[page.field];
    if (value == null || value === "" || (Array.isArray(value) && value.length === 0)) {
        return <Empty title={`No ${page.id} data yet`} />;
    }
    return (
        <p className="text-xs text-fg-muted">
            {Array.isArray(value) ? `${value.length} rows` : "loaded"}
        </p>
    );
}

// Pages that have a real screen; the rest keep the placeholder until their task lands.
const screens: Partial<Record<Page["id"], RouteComponent>> = {
    overview: Overview,
    plugins: Plugins,
    calls: Calls,
    skills: Skills,
    stats: Stats,
    graph: Graph,
    hosts: Hosts,
    config: Config,
    services: Services,
    worktrees: Worktrees,
};

const root = createRootRoute({ component: Shell, notFoundComponent: NotFound });
const index = createRoute({
    getParentRoute: () => root,
    path: "/",
    beforeLoad: () => {
        throw redirect({ to: `/${PAGES[0].id}` });
    },
});
const pageRoutes = PAGES.map((page) =>
    createRoute({
        getParentRoute: () => root,
        path: page.id,
        component: screens[page.id] ?? (() => <PagePlaceholder page={page} />),
    }),
);

export const routeTree = root.addChildren([index, ...pageRoutes]);

// Hash history: the page is served as one static file and `?sample` lives in the real
// query string, which a path-based router would drop on the first navigation.
export const createAppRouter = (history: RouterHistory = createHashHistory()) =>
    createRouter({ routeTree, history });
