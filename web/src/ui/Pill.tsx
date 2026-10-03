// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { ReactNode } from "react";

const tones = {
    ok: "border-success/40 bg-success/15 text-success-fg",
    warn: "border-warn/40 bg-warn/15 text-warn-fg",
    fail: "border-delta/40 bg-delta/15 text-delta-fg",
    info: "border-accent/40 bg-accent/15 text-accent-fg",
    muted: "border-border bg-surface-2 text-fg-muted",
};

export type PillTone = keyof typeof tones;

// The dot carries the tone for people who cannot tell the colours apart; the label says it.
export function Pill({
    tone = "muted",
    dot = false,
    children,
}: {
    tone?: PillTone;
    dot?: boolean;
    children: ReactNode;
}) {
    return (
        <span
            className={`inline-flex h-5 items-center gap-1.5 rounded-full border px-2 text-2xs font-semibold whitespace-nowrap ${tones[tone]}`}
        >
            {dot && <span aria-hidden="true" className="size-1.5 rounded-full bg-current" />}
            {children}
        </span>
    );
}
