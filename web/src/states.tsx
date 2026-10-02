import type { ReactNode } from "react";

// The four states every page shares; pages pick one instead of drawing their own.
function Panel({ children, ...aria }: { children: ReactNode; role: string; "aria-busy"?: true }) {
    return (
        <div {...aria} className="glass flex flex-col gap-1 p-4 text-xs">
            {children}
        </div>
    );
}

export function Loading() {
    return (
        <Panel role="status" aria-busy>
            <span className="sr-only">Loading</span>
            {[3 / 4, 1, 1 / 2].map((w) => (
                <div
                    key={w}
                    aria-hidden="true"
                    className="h-4 animate-pulse rounded-sm bg-surface-3"
                    style={{ width: `${w * 100}%` }}
                />
            ))}
        </Panel>
    );
}

export function Empty({ title, hint }: { title: string; hint?: string }) {
    return (
        <Panel role="status">
            <strong>{title}</strong>
            {hint && <span className="text-fg-muted">{hint}</span>}
        </Panel>
    );
}

export function ErrorState({ message }: { message: string }) {
    return (
        <Panel role="alert">
            <strong className="text-danger-fg">Something went wrong</strong>
            <span className="text-fg-muted">{message}</span>
        </Panel>
    );
}

export function Offline() {
    return (
        <Panel role="status">
            <strong className="text-warn-fg">Offline</strong>
            <span className="text-fg-muted">
                The connection to rtok was lost. The last data stays on screen and updates resume
                when it returns.
            </span>
        </Panel>
    );
}
