// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { focusRing } from "./cx";

// The 44px hit area is only for touch; a mouse keeps the compact control from the design.
export function Switch({
    checked,
    onCheckedChange,
    label,
    disabled = false,
}: {
    checked: boolean;
    onCheckedChange?: (next: boolean) => void;
    label: string;
    disabled?: boolean;
}) {
    return (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            aria-label={label}
            disabled={disabled}
            onClick={() => onCheckedChange?.(!checked)}
            className={`${focusRing} group inline-flex min-h-7 min-w-9 shrink-0 cursor-pointer items-center justify-center rounded-md pointer-coarse:min-h-11 pointer-coarse:min-w-11 disabled:cursor-not-allowed disabled:opacity-40`}
        >
            <span
                aria-hidden="true"
                className="relative h-5 w-9 rounded-full border border-border-strong bg-surface-3 transition-colors group-aria-checked:border-accent group-aria-checked:bg-accent/30 after:absolute after:top-0.5 after:left-0.5 after:size-3.5 after:rounded-full after:bg-fg-muted after:transition-transform group-aria-checked:after:translate-x-4 group-aria-checked:after:bg-accent-fg"
            />
        </button>
    );
}
