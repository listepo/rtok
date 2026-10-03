// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { type RefObject, useEffect, useRef } from "react";
import type { Scene } from "./scene";
import { Stage } from "./stage3d";
import type { Positions } from "./useLayout";
import { prefersReducedMotion, type ViewApi, type ViewEvents } from "./webgl";

export interface View3DProps extends ViewEvents {
    scene: Scene;
    positions: Positions;
    api: RefObject<ViewApi | null>;
    /** The 3D view cannot run (no context, or it was lost): the page switches to 2D and says why. */
    onUnavailable(reason: string): void;
}

/** The React shell around [`Stage`]: it builds the stage once, feeds it scenes, and gives it back on unmount. */
export default function Scene3D({
    scene,
    positions,
    api,
    select,
    menu,
    hover,
    onUnavailable,
}: View3DProps) {
    const host = useRef<HTMLDivElement>(null);
    const stage = useRef<Stage | null>(null);
    // The stage is created once; handlers it calls always see the latest props.
    const events = useRef({ select, menu, hover, onUnavailable });
    events.current = { select, menu, hover, onUnavailable };

    useEffect(() => {
        let made: Stage;
        try {
            made = new Stage(
                host.current!,
                {
                    select: (id) => events.current.select(id),
                    menu: (id, x, y) => events.current.menu(id, x, y),
                    hover: (h) => events.current.hover(h),
                },
                positions,
                prefersReducedMotion(),
                (why) => events.current.onUnavailable(why),
            );
        } catch (e) {
            events.current.onUnavailable(e instanceof Error ? e.message : "WebGL is not available");
            return;
        }
        stage.current = made;
        api.current = made;
        return () => {
            made.dispose();
            stage.current = null;
            api.current = null;
        };
    }, [positions, api]);

    useEffect(() => stage.current?.setScene(scene), [scene]);

    return <div ref={host} className="size-full min-h-72" data-testid="graph-3d" />;
}
