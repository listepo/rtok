// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ReactElement } from "react";
import { richSnapshot } from "./fixtures";
import { serve, withData } from "./storyData";
import { Usage } from "./Usage";
import {
    usageByModel,
    usageEmpty,
    usageFailed,
    usageLogs,
    usagePage,
    usageReading,
} from "./usageFixtures";

const answering = (agent_usage: typeof richSnapshot.agent_usage) =>
    serve({ ...richSnapshot, agent_usage });

const meta = { title: "Pages/Usage" } satisfies Meta;
export default meta;

const page = (connect = serve(richSnapshot)): StoryObj => ({
    render: (): ReactElement => <Usage />,
    decorators: [withData(connect)],
});

export const UsageDefault = page();
export const UsageLight: StoryObj = { ...UsageDefault, globals: { theme: "light" } };
// `--source logs`: no rtok columns, and one host whose files could not be read.
export const UsageLogsOnly = page(answering(usagePage(usageLogs)));
export const UsageByModel = page(answering(usagePage(usageByModel)));
export const UsageNothingRecorded = page(
    answering(usagePage(usageEmpty, "rtok agents usage: through rtok, no usage recorded (UTC)\n")),
);
export const UsageReading = page(answering(usageReading));
export const UsageFailed = page(answering(usageFailed));
