// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Inline SVG sparkline.
export function Sparkline({
    values,
    label,
    width = 96,
    height = 28,
    area = true,
}: {
    values: readonly number[];
    label: string;
    width?: number;
    height?: number;
    area?: boolean;
}) {
    if (values.length < 2) return <svg width={width} height={height} aria-hidden="true" />;
    const max = Math.max(...values);
    const min = Math.min(...values);
    const range = max - min || 1;
    const line = values
        .map((v, i) => {
            const x = (i / (values.length - 1)) * width;
            const y = height - 2 - ((v - min) / range) * (height - 4);
            return `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
        })
        .join("");
    return (
        <svg
            viewBox={`0 0 ${width} ${height}`}
            width={width}
            height={height}
            preserveAspectRatio="none"
            role="img"
            aria-label={label}
            className="overflow-visible text-accent-fg"
        >
            {area && (
                <path
                    d={`${line}L${width} ${height}L0 ${height}Z`}
                    fill="currentColor"
                    opacity="0.12"
                />
            )}
            <path
                d={line}
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinejoin="round"
                strokeLinecap="round"
                vectorEffect="non-scaling-stroke"
            />
        </svg>
    );
}
