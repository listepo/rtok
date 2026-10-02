import type { ReactNode } from "react";
import { useConnection, useSnapshot } from "../api/query";
import type { Snapshot } from "../api/snapshot.gen";
import { Loading } from "../states";

/** Renders `children` once a snapshot exists; the shell already shows the offline banner. */
export function WithSnapshot({ children }: { children: (snap: Snapshot) => ReactNode }) {
    const connection = useConnection();
    const { data } = useSnapshot();
    if (data) return children(data);
    return connection === "closed" ? null : <Loading />;
}

export function Toolbar({ children }: { children: ReactNode }) {
    return (
        <div className="glass flex flex-wrap items-end gap-x-4 gap-y-2 px-3 py-2.5">{children}</div>
    );
}

export function Split({ list, detail }: { list: ReactNode; detail: ReactNode }) {
    return (
        <div className="grid min-w-0 grid-cols-1 gap-3 lg:grid-cols-[minmax(0,1fr)_380px]">
            {list}
            {detail}
        </div>
    );
}

export function Kv({ rows }: { rows: readonly (readonly [string, ReactNode])[] }) {
    return (
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
            {rows.map(([k, v]) => (
                <div key={k} className="contents">
                    <dt className="text-fg-subtle">{k}</dt>
                    <dd className="min-w-0 break-words">{v}</dd>
                </div>
            ))}
        </dl>
    );
}

export function Count({ children }: { children: ReactNode }) {
    return <span className="ml-auto text-2xs text-fg-subtle">{children}</span>;
}
