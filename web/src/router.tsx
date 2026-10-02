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
import { Graph } from "./pages/Graph";
import { Hosts } from "./pages/Hosts";
import { Overview } from "./pages/Overview";
import { Plugins } from "./pages/Plugins";
import { Services } from "./pages/Services";
import { Skills } from "./pages/Skills";
import { Stats } from "./pages/Stats";
import { Worktrees } from "./pages/Worktrees";
import { Doctor } from "./pages/Doctor";
import { Logs } from "./pages/Logs";
import { Sessions } from "./pages/Sessions";
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
