// @vitest-environment happy-dom
import { cleanup, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, test } from "vitest";
import { connectSample } from "../api/sample";
import { richSnapshot } from "./fixtures";
import { hostNote, moduleState } from "./Hosts";
import { sourceTone } from "./Config";
import { serviceCommand } from "./Services";
import { matchesSkill, skillWarnings } from "./Skills";
import { costTotals } from "./Stats";
import { responsive } from "./parts";
import { mount, serving } from "./testHelpers";
import { parseStats } from "./text";
import { skillRows, statsText } from "./textFixtures";
import { baseName } from "./Worktrees";

afterEach(cleanup);

const rowsOf = (name: string) =>
    within(screen.getByRole("table", { name })).getAllByRole("row").slice(1);

describe("page logic", () => {
    test("skill warnings and filters", () => {
        const [worktrees, release] = skillRows;
        expect(skillWarnings(worktrees!)).toEqual(["long description"]);
        expect(skillWarnings(release!)).toEqual(["body > 8 KB", "never invoked"]);
        expect(matchesSkill(release!, true, "")).toBe(true);
        expect(matchesSkill(worktrees!, true, "")).toBe(false);
        expect(matchesSkill(worktrees!, false, "PROJECT")).toBe(false);
    });

    test("cost totals prefer the report's own total, else sum the priced rows", () => {
        expect(costTotals(parseStats(statsText))).toEqual({ cost: 15.17, saved: 3.55 });
        const noTotal = parseStats(statsText.replace(/^cost total.*\n/m, ""));
        expect(costTotals(noTotal).cost).toBeCloseTo(15.17);
    });

    test("host and module states", () => {
        expect(moduleState("not installed --plugin")).toEqual({
            tone: "warn",
            label: "not installed",
            rest: "--plugin",
        });
        expect(moduleState("installed").tone).toBe("ok");
        expect(moduleState("weird").tone).toBe("muted");
        expect(hostNote("not found").tone).toBe("muted");
        expect(hostNote("").label).toBe("present");
    });

    test("narrow screens keep only the named columns", () => {
        const col = (id: string) => ({ id, header: id, cell: () => id });
        const cols = [col("a"), col("b"), col("c")];
        expect(responsive(cols, true, ["a"])).toHaveLength(3);
        expect(responsive(cols, false, ["a", "c"]).map((c) => c.id)).toEqual(["a", "c"]);
    });

    test("config layers, service commands and worktree names", () => {
        expect([sourceTone("default"), sourceTone("flag"), sourceTone("user")]).toEqual([
            "muted",
            "warn",
            "info",
        ]);
        expect(
            serviceCommand({ name: "web", running: false, pid: null, uptimeSecs: null, log: null }),
        ).toBe("rtok demon start web");
        expect(baseName("~/a/b/rtok-t1/")).toBe("rtok-t1");
    });
});

describe("skills", () => {
    test("lists, filters to never-invoked and shows the selected detail", async () => {
        mount(serving(richSnapshot), "/skills");
        expect(await screen.findByText(/4 listed/)).toBeTruthy();
        expect(rowsOf("skills")).toHaveLength(4);
        fireEvent.click(screen.getByRole("switch", { name: "never invoked only" }));
        expect(rowsOf("skills")).toHaveLength(1);
        fireEvent.click(rowsOf("skills")[0]!);
        expect(screen.getByText("body > 8 KB")).toBeTruthy();
    });

    test("says so when the host lists none", async () => {
        mount(serving({ ...richSnapshot, skills: { header: "", rows: [] } }), "/skills");
        expect(await screen.findByText("No skills listed")).toBeTruthy();
    });
});

describe("stats", () => {
    test("charts the cost and cache tables and keeps the other lines visible", async () => {
        mount(serving(richSnapshot), "/stats");
        expect(await screen.findByRole("table", { name: "cost per model" })).toBeTruthy();
        expect(rowsOf("cost per model")).toHaveLength(4);
        expect(rowsOf("cache health")).toHaveLength(3);
        expect(screen.getByRole("list", { name: "cache busts" }).children).toHaveLength(2);
        expect(screen.getByText("$15.17")).toBeTruthy();
        expect(screen.getByText(/^sub-agents 3 sessions/)).toBeTruthy();
    });

    test("raw text shows the verbatim string and back", async () => {
        mount(serving(richSnapshot), "/stats");
        fireEvent.click(await screen.findByRole("button", { name: "raw text" }));
        expect(screen.getByText(/^sessions 42 compact 3/).tagName).toBe("PRE");
        fireEvent.click(screen.getByRole("button", { name: "raw text" }));
        expect(screen.getByRole("table", { name: "cost per model" })).toBeTruthy();
    });

    test("a null field is an alert naming the command, never fake data", async () => {
        mount(serving({ ...richSnapshot, stats: null }), "/stats");
        expect((await screen.findByRole("alert")).textContent).toContain("rtok stats --price");
    });
});

describe("graph", () => {
    test("shows the index health, pending files and dead symbols", async () => {
        mount(serving(richSnapshot), "/graph");
        expect(await screen.findByText("pad_right")).toBeTruthy();
        expect(screen.getByText("src/web/model.rs")).toBeTruthy();
        expect(screen.getByText(/195 more/)).toBeTruthy();
        expect(rowsOf("dead symbols")).toHaveLength(3);
    });

    test("null means the feature is off, not an error alert", async () => {
        mount(serving({ ...richSnapshot, graph: null }), "/graph");
        expect(await screen.findByText("Graph page off")).toBeTruthy();
        expect(screen.queryByRole("alert")).toBeNull();
    });
});

describe("hosts", () => {
    test("renders one card per host variant", async () => {
        mount(serving(richSnapshot), "/hosts");
        expect(await screen.findByRole("heading", { name: "Claude Code" })).toBeTruthy();
        expect(screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent)).toEqual([
            "Claude Code",
            "Codex",
            "Cursor",
            "Gemini CLI",
            "Zed",
        ]);
        expect(screen.getByText(/nothing of rtok here/)).toBeTruthy();
    });

    test("the first probe shows a loading state", async () => {
        mount(serving({ ...richSnapshot, hosts: "probing hosts…\n" }), "/hosts");
        expect(await screen.findByText(/probing hosts/)).toBeTruthy();
    });
});

describe("config", () => {
    test("groups keys and filters by layer", async () => {
        mount(serving(richSnapshot), "/config");
        expect(await screen.findByText("effective config")).toBeTruthy();
        expect(screen.getAllByRole("region", { name: /^(core|proxy|web)$/ })).toHaveLength(3);
        fireEvent.click(screen.getByRole("button", { name: "flag 1" }));
        expect(screen.getByText("1 keys")).toBeTruthy();
        fireEvent.change(screen.getByRole("searchbox", { name: "Filter config" }), {
            target: { value: "zzz" },
        });
        expect(screen.getByText("No key matches")).toBeTruthy();
    });

    test("?sample shows the config page too", async () => {
        mount(connectSample, "/config");
        expect(await screen.findByText("12 keys")).toBeTruthy();
    });
});

describe("services", () => {
    test("one card per service and the exporter tiles", async () => {
        mount(serving(richSnapshot), "/services");
        expect(await screen.findByText("2 of 3 running")).toBeTruthy();
        expect(screen.getByText("rtok demon start mcp")).toBeTruthy();
        expect(screen.getByText("18 pending")).toBeTruthy();
        expect(screen.getByText(/otel exported 120 calls/)).toBeTruthy();
    });
});

describe("worktrees", () => {
    test("filters by state and shows the selected worktree's detail", async () => {
        mount(serving(richSnapshot), "/worktrees");
        expect(
            await screen.findByText("5 worktrees: 167 MB source, 12.1 GB build cache"),
        ).toBeTruthy();
        expect(rowsOf("worktrees")).toHaveLength(5);
        fireEvent.click(screen.getByRole("button", { name: "dirty 1" }));
        expect(rowsOf("worktrees")).toHaveLength(1);
        fireEvent.click(rowsOf("worktrees")[0]!);
        expect(screen.getByText("1a2b3c4d claude")).toBeTruthy();
    });

    test("outside a git repository the page says so", async () => {
        mount(serving({ ...richSnapshot, worktrees: "not a git repository\n" }), "/worktrees");
        expect(await screen.findByText("not a git repository")).toBeTruthy();
    });
});
