// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// @vitest-environment happy-dom
import { cleanup, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { richSnapshot } from "./fixtures";
import { mount, serving } from "./testHelpers";
import { periodKind, unpricedTokens, usageLines, usd } from "./Usage";
import {
    usageBoth,
    usageByModel,
    usageEmpty,
    usageFailed,
    usageLogs,
    usagePage,
    usageReading,
} from "./usageFixtures";

afterEach(cleanup);

const rowsOf = (name: string) =>
    within(screen.getByRole("table", { name })).getAllByRole("row").slice(1);

const withUsage = (agent_usage: typeof richSnapshot.agent_usage) =>
    serving({ ...richSnapshot, agent_usage });

describe("usage helpers", () => {
    test("dollars read like the CLI's: unknown is a dash, free is $0.00", () => {
        expect(usd(null)).toBe("-");
        expect(usd(0)).toBe("$0.00");
        expect(usd(450.63)).toBe("$450.63");
        expect(usd(1234.5)).toBe("$1,234.50");
        expect(usd(-18.4)).toBe("-$18.40");
        expect(usd(-0.004)).toBe("$0.00");
    });

    test("the table lines are the agents, or the models when the report grouped by model", () => {
        expect(usageLines(usageBoth).map((l) => l.label)).toEqual([
            "Claude Code",
            "Codex",
            "Gemini CLI",
        ]);
        expect(usageLines(usageBoth)[0]).toMatchObject({ coverage: 0.58, savedTokens: 4_100_000 });
        expect(usageLines(usageByModel).map((l) => l.label)).toEqual([
            "claude-sonnet-5-5",
            "gpt-5.5",
            "claude-haiku-4-5",
            "gemini-3-pro-preview",
        ]);
        expect(usageLines(usageLogs)[0]).toMatchObject({ throughRtok: null, savedUsd: null });
    });

    test("the unpriced tokens and the period kind come from the report", () => {
        expect(unpricedTokens(usageBoth)).toBe(400_000);
        expect(unpricedTokens(usageEmpty)).toBe(0);
        expect(periodKind(usageBoth)).toBe("Monthly");
        expect(
            periodKind({
                ...usageBoth,
                periods: [{ ...usageBoth.periods[0]!, period: "2026-10-03" }],
            }),
        ).toBe("Daily");
    });

    test("the fixture rows add up the way the Rust report's do", () => {
        const sum = (rows: { tokens: number }[]) => rows.reduce((s, r) => s + r.tokens, 0);
        expect(sum(usageBoth.agents)).toBe(usageBoth.totals.tokens);
        expect(sum(usageBoth.periods)).toBe(usageBoth.totals.tokens);
    });
});

describe("usage page", () => {
    test("shows the totals, the unpriced warning and one row per agent", async () => {
        mount(serving(richSnapshot), "/usage");
        expect(await screen.findByText("$450.63")).toBeTruthy();
        expect(screen.getByText("incomplete: 1 unpriced")).toBeTruthy();
        expect(screen.getByRole("alert").textContent).toContain("1 model has no price");
        expect(screen.getByText("gemini-3-pro-preview")).toBeTruthy();
        const rows = rowsOf("usage per agent");
        expect(rows).toHaveLength(3);
        expect(rows[0]!.textContent).toContain("Claude Code");
        expect(rows[0]!.textContent).toContain("$412.73");
        expect(rows[0]!.textContent).toContain("58%");
        // An agent without a priced model shows a dash, not $0.00.
        expect(rows[2]!.textContent).not.toContain("$0.00");
        expect(rowsOf("monthly usage")).toHaveLength(3);
    });

    test("a logs-only report has no rtok columns and names the skipped host", async () => {
        mount(withUsage(usagePage(usageLogs)), "/usage");
        await screen.findByRole("table", { name: "usage per agent" });
        expect(screen.queryByText("coverage")).toBeNull();
        expect(screen.queryByText("rtok saved")).toBeNull();
        expect(screen.getByLabelText("unreadable hosts").textContent).toContain("opencode");
    });

    test("--by model swaps the agent table for the models", async () => {
        mount(withUsage(usagePage(usageByModel)), "/usage");
        expect(await screen.findAllByRole("row")).not.toHaveLength(0);
        expect(rowsOf("usage per model")).toHaveLength(4);
        expect(screen.queryByRole("table", { name: "usage per agent" })).toBeNull();
    });

    test("nothing recorded, still reading and a failed read each say so", async () => {
        const { unmount } = mount(
            withUsage(
                usagePage(usageEmpty, "rtok agents usage: through rtok, no usage recorded (UTC)\n"),
            ),
            "/usage",
        );
        expect(await screen.findByText("No usage recorded")).toBeTruthy();
        unmount();

        const reading = mount(withUsage(usageReading), "/usage");
        expect((await screen.findByRole("status")).textContent).toBe("reading usage…");
        reading.unmount();

        mount(withUsage(usageFailed), "/usage");
        expect((await screen.findByRole("alert")).textContent).toContain("unknown time zone");
    });
});
