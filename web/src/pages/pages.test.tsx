// @vitest-environment happy-dom
import { cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { connectSample } from "../api/sample";
import { ago, compact, pct } from "./format";
import { richSnapshot } from "./fixtures";
import { callBuckets, doctorChecks, overview, tokensOf } from "./model";
import { matchesPlugin } from "./Plugins";
import { matchesCall } from "./Calls";
import { mount, serving } from "./testHelpers";

afterEach(cleanup);

describe("page logic", () => {
    test("formatting", () => {
        expect([
            compact(999),
            compact(1_500),
            compact(25_000),
            compact(3_400_000),
            compact(null),
        ]).toEqual(["999", "1.5k", "25k", "3.4M", "-"]);
        expect([pct(0.256), pct(NaN)]).toEqual(["25.6%", "-"]);
    });

    test("overview sums measured plugins only and ranks them by saving", () => {
        const o = overview(richSnapshot);
        expect([o.estBefore, o.estAfter, o.saved]).toEqual([180_000, 70_000, 110_000]);
        expect(o.measured.map((m) => m.plugin.id)).toEqual(["shell", "read"]);
        expect(o.deltaPct).toBeCloseTo(110_000 / 180_000);
        expect([o.failed, o.p95, o.live, o.hosts, o.enabled]).toEqual([1, 3_400, 1, 2, 3]);
    });

    test("overview of an empty frame has no NaN leaks beyond the formatted dash", () => {
        const o = overview({ ...richSnapshot, plugins: [], calls: [], sessions: [] });
        expect([o.saved, o.failed, o.p95, o.enabled]).toEqual([0, 0, null, 0]);
        expect(pct(o.deltaPct)).toBe("-");
    });

    test("tokens of a call count cache and output, and are unknown without usage", () => {
        const [proxy, mcp] = richSnapshot.calls;
        expect(tokensOf(proxy!)).toBe(41_200);
        expect(tokensOf(mcp!)).toBeNull();
    });

    test("calls land in buckets by surface and failures are counted once", () => {
        const { buckets } = callBuckets(richSnapshot.calls);
        expect(buckets).toHaveLength(24);
        const sum = (k: "hook" | "mcp" | "proxy" | "err") => buckets.reduce((s, b) => s + b[k], 0);
        expect([sum("hook"), sum("mcp"), sum("proxy"), sum("err")]).toEqual([4, 3, 1, 1]);
        // The newest call sits in the last bucket, the oldest in the first.
        expect([buckets[0]!.mcp, buckets[23]!.proxy]).toEqual([1, 1]);
        expect(callBuckets([]).buckets.every((b) => b.hook + b.mcp + b.proxy === 0)).toBe(true);
    });

    test("doctor rows name the field they read and a failed probe is a failure", () => {
        const states = doctorChecks(richSnapshot.doctor).map((c) => c.st);
        expect(states.filter((s) => s === "pass")).toHaveLength(4);
        expect(states.filter((s) => s === "warn")).toHaveLength(3);
        expect(states.filter((s) => s === "skip")).toHaveLength(2);
        expect(doctorChecks(null)).toMatchObject([{ st: "fail", field: "Snapshot.doctor = null" }]);
        const bare = { ...richSnapshot.doctor!, hooks_total: 0, mcp: [], proxy: "" };
        expect(
            doctorChecks(bare)
                .slice(0, 3)
                .map((c) => c.st),
        ).toEqual(["fail", "warn", "warn"]);
    });

    test("ages read in the largest whole unit", () => {
        expect([ago(95, 100), ago(0, 600), ago(0, 7_300), ago(0, 200_000), ago(500, 100)]).toEqual([
            "5s ago",
            "10m ago",
            "2h ago",
            "2d ago",
            "0s ago",
        ]);
    });

    test("plugin and call filters", () => {
        const [shell, , graph, ledger] = richSnapshot.plugins;
        expect([shell, graph, ledger].map((p) => matchesPlugin(p!, "on", ""))).toEqual([
            true,
            false,
            true,
        ]);
        expect(matchesPlugin(graph!, "off", "")).toBe(true);
        expect(matchesPlugin(ledger!, "saves", "")).toBe(false);
        expect(matchesPlugin(shell!, "all", "NOISY")).toBe(true);
        const failed = richSnapshot.calls.find((c) => !c.ok)!;
        expect(matchesCall(failed, "hook", "failed", "")).toBe(true);
        expect(matchesCall(failed, "mcp", "any", "")).toBe(false);
        expect(matchesCall(failed, "all", "ok", "")).toBe(false);
        expect(matchesCall(failed, "all", "any", "pretool")).toBe(true);
    });
});

describe("overview", () => {
    test("shows the alert, the KPIs and the ranked savings", async () => {
        mount(serving(richSnapshot), "/overview");
        expect((await screen.findByRole("alert")).textContent).toContain("context window 91%");
        expect(screen.getByText("Δtok %").nextElementSibling?.textContent).toContain("61.1%");
        const table = await screen.findByRole("table", { name: "savings by plugin" });
        expect(
            within(table)
                .getAllByRole("row")
                .slice(1)
                .map((r) => within(r).getAllByRole("cell")[0]?.textContent),
        ).toEqual(["shell", "read"]);
    });

    test("draws the calls chart with its legend, the doctor card and the recent sessions", async () => {
        mount(serving(richSnapshot), "/overview");
        const chart = await screen.findByRole("img", { name: /^Calls over time: 8 calls/ });
        expect(chart.getAttribute("aria-label")).toContain("1 failed");
        expect(screen.getByText("hook 4")).toBeTruthy();
        const doctor = screen.getByRole("region", { name: "doctor" });
        expect(within(doctor).getByText("MCP tool search")).toBeTruthy();
        expect(within(doctor).queryByText("hooks installed")).toBeNull();
        const sessions = screen.getByRole("region", { name: "recent sessions" });
        expect(
            within(sessions)
                .getAllByRole("listitem")
                .map((li) => li.textContent),
        ).toEqual([expect.stringContaining("live"), expect.stringContaining("ended")]);
        expect(screen.getByRole("img", { name: "3 of 4 plugins enabled" })).toBeTruthy();
    });

    test("renders a failed doctor probe and an empty ledger instead of zeros", async () => {
        mount(serving({ ...richSnapshot, doctor: null, calls: [], sessions: [] }), "/overview");
        const doctor = await screen.findByRole("region", { name: "doctor" });
        expect(within(doctor).getByText("doctor probe")).toBeTruthy();
        expect(screen.getByText("No calls yet")).toBeTruthy();
        expect(screen.getByText("No sessions yet")).toBeTruthy();
    });

    test("says so when nothing is measured", async () => {
        mount(serving({ ...richSnapshot, plugins: [] }), "/overview");
        expect(await screen.findByText("No measured savings yet")).toBeTruthy();
    });
});

describe("plugins", () => {
    test("filters by group and by text", async () => {
        mount(serving(richSnapshot), "/plugins");
        const table = await screen.findByRole("table", { name: "plugins" });
        expect(within(table).getAllByRole("row")).toHaveLength(5);
        fireEvent.click(screen.getByRole("button", { name: "disabled" }));
        await waitFor(() => expect(within(table).getAllByRole("row")).toHaveLength(2));
        fireEvent.click(screen.getByRole("button", { name: "all" }));
        fireEvent.change(screen.getByRole("searchbox", { name: "Filter plugins" }), {
            target: { value: "nothing-like-this" },
        });
        expect(await screen.findByText("No plugin matches")).toBeTruthy();
    });

    test("the switch round-trips through the /ws contract (sample server)", async () => {
        mount(connectSample, "/plugins");
        const toggle = await screen.findByRole("switch", { name: "toggle shell" });
        expect(toggle.getAttribute("aria-checked")).toBe("true");
        fireEvent.click(toggle);
        await waitFor(() =>
            expect(
                screen.getByRole("switch", { name: "toggle shell" }).getAttribute("aria-checked"),
            ).toBe("false"),
        );
    });

    test("a switch inside a row does not need the row's keys", async () => {
        mount(serving(richSnapshot), "/plugins");
        const toggle = await screen.findByRole("switch", { name: "toggle shell" });
        // Space on the switch must stay the switch's: the row only reacts to its own keys.
        expect(fireEvent.keyDown(toggle, { key: " " })).toBe(true);
    });
});

describe("calls", () => {
    test("selecting a failed call shows its error and details", async () => {
        mount(serving(richSnapshot), "/calls");
        const table = await screen.findByRole("table", { name: "calls" });
        const row = within(table)
            .getAllByRole("row")
            .find((r) => r.textContent?.includes("PreToolUse"))!;
        fireEvent.click(row);
        expect((await screen.findByRole("alert")).textContent).toContain("index not built");
        expect(screen.getByText("ref_id").nextElementSibling?.textContent).toBe("-");
    });

    test("expand fetches the archived output and filters it", async () => {
        mount(connectSample, "/calls");
        const table = await screen.findByRole("table", { name: "calls" });
        // The sample frame has call #13 (the graph `symbol` lookup) with archive id `sample-archive-graph`.
        fireEvent.click(
            within(table)
                .getAllByRole("row")
                .find((r) => r.textContent?.includes("symbol"))!,
        );
        fireEvent.click(await screen.findByRole("button", { name: "expand sample-archive-graph" }));
        const out = await screen.findByText("sample payload for sample-archive-graph");
        expect(out.tagName).toBe("PRE");
        fireEvent.change(screen.getByRole("searchbox", { name: "Filter expanded output" }), {
            target: { value: "zzz" },
        });
        await waitFor(() => expect(screen.getByText("0 lines")).toBeTruthy());
    });

    test("says so when the ledger is empty", async () => {
        mount(serving({ ...richSnapshot, calls: [] }), "/calls");
        expect(await screen.findByText("No calls yet")).toBeTruthy();
    });
});
