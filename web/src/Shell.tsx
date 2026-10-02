import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { useEffect, useRef } from "react";
import logo from "../assets/logo.svg";
import { useConnection, useSnapshot } from "./api/query";
import { Orb } from "./Orb";
import { PAGES, type Page } from "./pages";
import { Empty, ErrorState, Offline } from "./states";
import { useTheme } from "./theme";
import { Icon } from "./ui/Icon";

const focusRing = "outline-none focus-visible:shadow-ring";
const navLink = `${focusRing} flex h-9 shrink-0 items-center gap-3 rounded-md px-2.5 text-xs text-fg-muted hover:bg-surface-2 hover:text-fg aria-[current=page]:bg-accent/15 aria-[current=page]:text-accent-fg`;

export function Shell() {
    const connection = useConnection();
    const { data } = useSnapshot();
    const { dark, toggle } = useTheme();
    const pathname = useRouterState({ select: (s) => s.location.pathname });
    const title = PAGES.find((p) => `/${p.id}` === pathname)?.id ?? "not found";
    const heading = useRef<HTMLHeadingElement>(null);
    const mounted = useRef(false);

    useEffect(() => {
        document.title = `${title} · rtok`;
        // A client-side route change moves no focus on its own; without this a keyboard or
        // screen-reader user stays on the link they pressed and hears nothing.
        if (mounted.current) heading.current?.focus();
        mounted.current = true;
    }, [title]);

    return (
        <>
            <a
                href="#main"
                className={`${focusRing} sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-accent focus:px-3 focus:py-2 focus:text-accent-on`}
            >
                Skip to content
            </a>
            <Orb />
            <div className="min-h-screen md:grid md:grid-cols-[200px_minmax(0,1fr)] lg:grid-cols-[232px_minmax(0,1fr)]">
                <nav
                    aria-label="Admin screens"
                    className="glass fixed inset-x-2 bottom-2 z-40 flex gap-0.5 overflow-x-auto p-1 md:sticky md:inset-auto md:top-0 md:m-3 md:h-[calc(100vh-1.5rem)] md:flex-col md:overflow-y-auto"
                >
                    <div className="hidden items-center gap-2 px-2.5 py-2 md:flex">
                        <img src={logo} alt="" width={22} height={22} className="rounded-sm" />
                        <span className="text-xs font-bold tracking-wordmark">RTOK</span>
                    </div>
                    {PAGES.map((p: Page) => (
                        <Link key={p.id} to={`/${p.id}`} className={navLink}>
                            <Icon name={p.id} />
                            {p.id}
                        </Link>
                    ))}
                </nav>
                <div className="flex min-w-0 flex-col">
                    <header className="flex items-center gap-2 px-3 py-3">
                        <h1
                            ref={heading}
                            tabIndex={-1}
                            className="min-w-0 flex-1 truncate text-base font-bold outline-none md:text-sm"
                        >
                            {title}
                        </h1>
                        <span className="text-2xs text-fg-subtle">{connection}</span>
                        <button
                            type="button"
                            onClick={toggle}
                            aria-pressed={!dark}
                            aria-label={dark ? "Switch to light theme" : "Switch to dark theme"}
                            className={`${focusRing} h-7 rounded-md px-2 text-xs text-fg-muted hover:bg-surface-2 hover:text-fg`}
                        >
                            {dark ? "Light" : "Dark"}
                        </button>
                    </header>
                    <main
                        id="main"
                        tabIndex={-1}
                        className="flex min-w-0 flex-1 flex-col gap-3 px-3 pt-1 pb-24 outline-none md:pb-6"
                    >
                        {connection === "closed" && <Offline />}
                        {data?.error && <ErrorState message={data.error} />}
                        <Outlet />
                    </main>
                </div>
            </div>
        </>
    );
}

export function NotFound() {
    return <Empty title="Page not found" hint="Pick a screen from the navigation." />;
}
