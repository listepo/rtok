import { useId, type ReactNode } from "react";

export function Panel({
    title,
    hint,
    children,
}: {
    title: string;
    hint?: string;
    children: ReactNode;
}) {
    const id = useId();
    return (
        <section aria-labelledby={id} className="glass flex min-w-0 flex-col">
            <header className="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 border-b border-border/70 px-4 py-3">
                <h2 id={id} className="text-sm font-bold">
                    {title}
                </h2>
                {hint && <p className="text-2xs text-fg-muted">{hint}</p>}
            </header>
            <div className="flex min-w-0 flex-col gap-4 p-4">{children}</div>
        </section>
    );
}
