// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { useEffect, useRef, useState } from "react";
import { startOrb } from "./orbGl";

// The CSS gradient is the first paint and the fallback: no WebGL, reduced motion or the
// `rtok-orb=off` switch all end up showing it instead of the canvas.
export function Orb() {
    const canvas = useRef<HTMLCanvasElement>(null);
    const [webgl, setWebgl] = useState(false);
    useEffect(() => (canvas.current ? startOrb(canvas.current, setWebgl) : undefined), []);
    return (
        <>
            <div
                data-testid="orb-fallback"
                hidden={webgl}
                aria-hidden="true"
                className="pointer-events-none fixed inset-0 -z-10 [opacity:var(--orb-opacity)] bg-[radial-gradient(40%_50%_at_78%_18%,rgb(var(--rtok-accent-rgb)/.16),transparent_70%),radial-gradient(30%_40%_at_88%_30%,rgb(var(--rtok-delta-rgb)/.08),transparent_70%)]"
            />
            <canvas
                ref={canvas}
                hidden={!webgl}
                aria-hidden="true"
                className="pointer-events-none fixed inset-0 -z-10 h-full w-full [opacity:var(--orb-opacity)]"
            />
        </>
    );
}
