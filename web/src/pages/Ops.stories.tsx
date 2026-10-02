// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import { sampleSnapshot } from "../api/sample";
import { Doctor } from "./Doctor";
import { Logs } from "./Logs";
import { Sessions } from "./Sessions";
import { serve, withData } from "./storyKit";

const sample = serve(sampleSnapshot);

export default { title: "Pages/Ops" } satisfies Meta;

const story = (render: () => React.JSX.Element, snapshot = sample, theme?: "light"): StoryObj => ({
    render,
    decorators: [withData(snapshot)],
    ...(theme && { globals: { theme } }),
});

export const SessionsDefault = story(() => <Sessions />);
export const SessionsLight = story(() => <Sessions />, sample, "light");
export const SessionsEmpty = story(() => <Sessions />, serve({ ...sampleSnapshot, sessions: [] }));

export const DoctorDefault = story(() => <Doctor />);
export const DoctorLight = story(() => <Doctor />, sample, "light");
export const DoctorProbeFailed = story(
    () => <Doctor />,
    serve({ ...sampleSnapshot, doctor: null }),
);

export const LogsDefault = story(() => <Logs />);
export const LogsLight = story(() => <Logs />, sample, "light");
export const LogsEmpty = story(() => <Logs />, serve({ ...sampleSnapshot, logs: [] }));
