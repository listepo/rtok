// TanStack Query wiring for the `/ws` stream (T310.3). The server pushes, so the snapshot
// lives in the query cache (written by `setQueryData`, never fetched) and every page reads
// it through `useSnapshot`.
import {
    QueryClient,
    QueryClientProvider,
    skipToken,
    useMutation,
    useQuery,
} from "@tanstack/react-query";
import { createContext, useContext, useEffect, useMemo, type ReactNode } from "react";
import type { SetRequest, Snapshot } from "./snapshot.gen";
import type { Connect, Connection, ConnectionState, Frame } from "./ws";

export const snapshotKey = ["snapshot"] as const;
export const connectionKey = ["connection"] as const;
export const messageKey = ["message"] as const;

export const EXPAND_TIMEOUT_MS = 10_000;

export interface Api {
    open(): void;
    close(): void;
    expand(id: string): Promise<string>;
    set(request: SetRequest): Promise<void>;
}

interface PendingExpand {
    id: string;
    resolve(text: string): void;
    reject(error: Error): void;
}

export function createApi(
    queryClient: QueryClient,
    connect: Connect,
    expandTimeoutMs = EXPAND_TIMEOUT_MS,
): Api {
    let connection: Connection | null = null;
    let pending: PendingExpand[] = [];

    const rejectAll = (reason: string) => {
        const failed = pending;
        pending = [];
        for (const p of failed) p.reject(new Error(reason));
    };

    const onFrame = (frame: Frame) => {
        switch (frame.type) {
            case "snapshot":
                queryClient.setQueryData<Snapshot>(snapshotKey, frame.snapshot);
                return;
            case "snapshot_error":
                // A failed tick has no page data; keep the last good one and surface the error.
                queryClient.setQueryData<Snapshot>(
                    snapshotKey,
                    (prev) => prev && { ...prev, error: frame.error },
                );
                return;
            case "expand": {
                const done = pending.filter((p) => p.id === frame.id);
                pending = pending.filter((p) => p.id !== frame.id);
                for (const p of done) p.resolve(frame.text);
                return;
            }
            case "message":
                queryClient.setQueryData<string>(messageKey, frame.text);
                // The server's refusals do not name the request they answer, so a message fails
                // every expand in flight instead of leaving it to the timeout.
                rejectAll(frame.text);
        }
    };

    const onState = (state: ConnectionState) => {
        queryClient.setQueryData<ConnectionState>(connectionKey, state);
        if (state === "closed") rejectAll("connection closed");
    };

    return {
        open() {
            connection ??= connect({ onState, onFrame });
        },
        close() {
            connection?.close();
            connection = null;
        },
        expand(id) {
            return new Promise<string>((resolve, reject) => {
                const entry: PendingExpand = {
                    id,
                    resolve: (text) => {
                        clearTimeout(timer);
                        resolve(text);
                    },
                    reject: (error) => {
                        clearTimeout(timer);
                        reject(error);
                    },
                };
                const timer = setTimeout(() => {
                    pending = pending.filter((p) => p !== entry);
                    reject(new Error(`expand ${id} timed out`));
                }, expandTimeoutMs);
                pending.push(entry);
                if (!connection?.send({ expand: id })) {
                    pending = pending.filter((p) => p !== entry);
                    entry.reject(new Error("not connected"));
                }
            });
        },
        async set(request) {
            if (!connection?.send({ set: request })) throw new Error("not connected");
        },
    };
}

const ApiContext = createContext<Api | null>(null);

export function DataProvider({ connect, children }: { connect: Connect; children: ReactNode }) {
    const queryClient = useMemo(() => new QueryClient(), []);
    const api = useMemo(() => createApi(queryClient, connect), [queryClient, connect]);
    useEffect(() => {
        api.open();
        return () => api.close();
    }, [api]);
    return (
        <QueryClientProvider client={queryClient}>
            <ApiContext value={api}>{children}</ApiContext>
        </QueryClientProvider>
    );
}

function useApi(): Api {
    const api = useContext(ApiContext);
    if (!api) throw new Error("DataProvider is missing");
    return api;
}

// `skipToken` makes these cache-only queries: no fetch, no retry, just what the socket wrote.
const pushed = (queryKey: readonly string[]) =>
    ({ queryKey, queryFn: skipToken, staleTime: Infinity }) as const;

export const useSnapshot = () => useQuery<Snapshot>(pushed(snapshotKey));

export const useConnection = (): ConnectionState =>
    useQuery<ConnectionState>(pushed(connectionKey)).data ?? "connecting";

export const useServerMessage = () => useQuery<string>(pushed(messageKey)).data;

export function useSetMutation() {
    const api = useApi();
    return useMutation({ mutationFn: (request: SetRequest) => api.set(request) });
}

export function useExpandMutation() {
    const api = useApi();
    return useMutation({ mutationFn: (id: string) => api.expand(id) });
}
