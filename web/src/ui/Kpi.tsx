import type { ReactNode } from "react";

const tones = { default: "", ok: "text-success-fg", warn: "text-warn-fg", fail: "text-delta-fg" };

export function Kpi({
    label,
    value,
    sub,
    viz,
    tone = "default",
}: {
    label: string;
    value: ReactNode;
    sub?: ReactNode;
    viz?: ReactNode;
    tone?: keyof typeof tones;
}) {
    return (
        <div className="glass flex min-w-0 flex-col gap-1.5 p-3">
            <span className="truncate text-2xs font-semibold tracking-kicker text-fg-subtle uppercase">
                {label}
            </span>
            <div className="min-w-0">
                <div
                    className={`text-xl leading-none font-semibold whitespace-nowrap ${tones[tone]}`}
                >
                    {value}
                </div>
                {sub && <div className="mt-1.5 truncate text-2xs text-fg-muted">{sub}</div>}
            </div>
            {viz && (
                <div className="mt-auto flex h-7 items-end pt-1 [&>svg]:h-7 [&>svg]:w-full">
                    {viz}
                </div>
            )}
        </div>
    );
}
