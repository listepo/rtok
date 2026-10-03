// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useId } from "react";
import { focusRing } from "./cx";

export function Search({
    value,
    onChange,
    label,
    placeholder,
}: {
    value: string;
    onChange: (next: string) => void;
    label: string;
    placeholder?: string;
}) {
    const id = useId();
    return (
        <div className="min-w-0">
            <label htmlFor={id} className="mb-1 block text-2xs font-semibold text-fg-muted">
                {label}
            </label>
            <input
                id={id}
                type="search"
                value={value}
                placeholder={placeholder}
                onChange={(e) => onChange(e.target.value)}
                className={`${focusRing} h-8 w-full rounded-md border border-border bg-bg px-2.5 text-xs text-fg placeholder:text-fg-subtle hover:border-border-strong disabled:cursor-not-allowed disabled:opacity-40`}
            />
        </div>
    );
}
