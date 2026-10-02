import {
    createHashHistory,
    createRootRoute,
    createRoute,
    createRouter,
    redirect,
    type RouterHistory,
} from "@tanstack/react-router";
import { useConnection, useSnapshot } from "./api/query";
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
        component: () => <PagePlaceholder page={page} />,
    }),
);

export const routeTree = root.addChildren([index, ...pageRoutes]);

// Hash history: the page is served as one static file and `?sample` lives in the real
// query string, which a path-based router would drop on the first navigation.
export const createAppRouter = (history: RouterHistory = createHashHistory()) =>
    createRouter({ routeTree, history });
