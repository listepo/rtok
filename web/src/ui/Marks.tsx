// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

export function MiniBars({
    values,
    label,
    width = 96,
    height = 28,
    className = "fill-accent-fg",
}: {
    values: readonly number[];
    label: string;
    width?: number;
    height?: number;
    className?: string;
}) {
    if (!values.length) return null;
    const max = Math.max(...values, 1);
    const bar = width / values.length;
    return (
        <svg
            viewBox={`0 0 ${width} ${height}`}
            width={width}
            height={height}
            preserveAspectRatio="none"
            role="img"
            aria-label={label}
        >
            {values.map((v, i) => {
                const h = (v / max) * height;
                return (
                    <rect
                        // Bars are positional: the series has no ids and never reorders.
                        key={i}
                        x={i * bar + 1}
                        y={height - h}
                        width={Math.max(1, bar - 2)}
                        height={h}
                        rx="1"
                        className={className}
                        opacity={0.45 + 0.55 * (v / max)}
                    />
                );
            })}
        </svg>
    );
}

// One dot per plugin, lit when enabled (echoes assets/logo.svg); disabled dots are coral so
// the state does not hang on brightness alone.
const tone = (p: { enabled: boolean; saves_tokens: boolean }) =>
    !p.enabled ? "bg-delta-fg/80" : p.saves_tokens ? "bg-accent-fg" : "bg-accent-fg/40";

export function Bitset({
    items,
}: {
    items: readonly { id: string; enabled: boolean; saves_tokens: boolean }[];
}) {
    const on = items.filter((p) => p.enabled).length;
    return (
        <div
            role="img"
            aria-label={`${on} of ${items.length} plugins enabled`}
            className="ml-auto grid w-max grid-cols-6 gap-1"
        >
            {items.map((p) => (
                <span
                    key={p.id}
                    title={`${p.id}: ${p.enabled ? "enabled" : "disabled"}`}
                    className={`size-2 rounded-full ${tone(p)}`}
                />
            ))}
        </div>
    );
}
