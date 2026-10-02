// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { ReactNode } from "react";
import { focusRing } from "./cx";

export function Chip({
    pressed,
    onPressedChange,
    disabled = false,
    children,
}: {
    pressed: boolean;
    onPressedChange?: (next: boolean) => void;
    disabled?: boolean;
    children: ReactNode;
}) {
    return (
        <button
            type="button"
            aria-pressed={pressed}
            disabled={disabled}
            onClick={() => onPressedChange?.(!pressed)}
            className={`${focusRing} inline-flex h-7 cursor-pointer items-center justify-center gap-1.5 rounded-full border border-border px-2.5 text-2xs font-semibold text-fg-muted hover:border-border-strong hover:text-fg aria-pressed:border-accent aria-pressed:bg-accent/15 aria-pressed:text-accent-fg disabled:cursor-not-allowed disabled:opacity-40`}
        >
            {children}
        </button>
    );
}
