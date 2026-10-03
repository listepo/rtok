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
import { Calls } from "./pages/Calls";
import { Config } from "./pages/Config";
import { Doctor } from "./pages/Doctor";
import { Graph } from "./pages/Graph";
import { Hosts } from "./pages/Hosts";
import { Logs } from "./pages/Logs";
import { Overview } from "./pages/Overview";
import { Plugins } from "./pages/Plugins";
import { Services } from "./pages/Services";
import { Sessions } from "./pages/Sessions";
import { Skills } from "./pages/Skills";
import { Stats } from "./pages/Stats";
import { Usage } from "./pages/Usage";
import { Worktrees } from "./pages/Worktrees";
import { PAGES, type Page } from "./pages";
import { NotFound, Shell } from "./Shell";

const screens: Record<Page["id"], RouteComponent> = {
    overview: Overview,
    sessions: Sessions,
    doctor: Doctor,
    logs: Logs,
    plugins: Plugins,
    calls: Calls,
    skills: Skills,
    stats: Stats,
    graph: Graph,
    hosts: Hosts,
    config: Config,
    services: Services,
    worktrees: Worktrees,
    usage: Usage,
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
        component: screens[page.id],
    }),
);

export const routeTree = root.addChildren([index, ...pageRoutes]);

// Hash history: the page is served as one static file and `?sample` lives in the real
// query string, which a path-based router would drop on the first navigation.
export const createAppRouter = (history: RouterHistory = createHashHistory()) =>
    createRouter({ routeTree, history });
