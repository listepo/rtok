// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { describe, expect, test } from "vitest";
import {
  configText,
  graphText,
  hostsText,
  servicesText,
  statsText,
  worktreesText,
} from "./textFixtures";
import {
  groupConfig,
  humanSecs,
  kvPairs,
  kvSpaced,
  matchesConfig,
  matchesWorktree,
  parseConfig,
  parseGraph,
  parseHosts,
  parseServices,
  parseStats,
  parseWorktrees,
} from "./text";

describe("line helpers", () => {
  test("key=value and key number pairs", () => {
    expect(kvPairs("usage input=1 hit=85.0%  median_context=9")).toEqual({
      input: "1",
      hit: "85.0%",
      median_context: "9",
    });
    expect(kvSpaced("sessions 42  compact 3  lines 18204")).toEqual({
      sessions: "42",
      compact: "3",
      lines: "18204",
    });
  });

  test("durations", () => {
    expect([humanSecs(59), humanSecs(125), humanSecs(18_342), humanSecs(null)]).toEqual([
      "0m 59s",
      "2m 5s",
      "5h 5m",
      "-",
    ]);
  });
});

describe("stats", () => {
  const v = parseStats(statsText);

  test("reads totals, usage, priced rows and the unpriced dash", () => {
    expect(v.totals.sessions).toBe("42");
    expect(v.usage.hit).toBe("85.0%");
    expect(v.priced.map((r) => r.model)).toEqual([
      "claude-sonnet-5",
      "claude-haiku-4-5",
      "gpt-5",
      "claude-opus-4-1",
    ]);
    expect(v.priced[0]).toMatchObject({ cost: 12.84, saved: 3.1, cacheRead: 7_203_311 });
    expect(v.priced[3]).toMatchObject({ cost: null, saved: null });
  });

  test("takes the cost total line instead of treating it as a model row", () => {
    expect(v.costTotal).toEqual({ cost: 15.17, saved: 3.55 });
  });

  test("reads cache health rows and busts", () => {
    expect(v.health).toHaveLength(3);
    expect(v.health[2]).toMatchObject({ turns: 17, busts: 2 });
    expect(v.busts).toEqual([
      { turn: "41", cause: "tools", cacheCreate: 48_120, cacheRead: 0 },
      { turn: "6", cause: "tools", cacheCreate: 31_000, cacheRead: 0 },
    ]);
  });

  test("keeps the sections it does not chart", () => {
    expect(v.other.map((l) => l.split(" ")[0])).toEqual(["sub-agents", "thinking"]);
  });

  test("an empty report parses to nothing and keeps the hint line", () => {
    const e = parseStats(
      "sessions 0  compact 0  checkpoint 0  no_checkpoint 0  lines 0  malformed 0\n\ncache health\nsession  turns  cache_read  cache_create  busts\nno usage rows (run `rtok proxy` first)\n",
    );
    expect([e.priced, e.health, e.busts]).toEqual([[], [], []]);
    expect(e.other).toEqual(["no usage rows (run `rtok proxy` first)"]);
  });
});

describe("graph", () => {
  test("reads the index health, pending files and dead symbols", () => {
    const v = parseGraph(graphText);
    expect([v.rows, v.files, v.watch, v.indexedAt]).toEqual([
      48_213,
      612,
      true,
      "2026-09-27 18:12:40",
    ]);
    expect(v.pending).toEqual(["src/web/model.rs", "src/tui/view.rs"]);
    expect(v.dead[0]).toEqual({
      path: "src/render.rs",
      line: "212",
      kind: "function",
      name: "pad_right",
    });
    expect(v.deadNote).toContain("195 more");
    expect(v.other).toEqual([]);
  });

  test("`none` and a failed scan are not symbols", () => {
    expect(
      parseGraph(
        "root /r\nrows 1\nfiles 1\npending 0\nwatch false\nindexed_at -\n\ndead symbols\n none\n",
      ).dead,
    ).toEqual([]);
    expect(parseGraph("root /r\n\ndead symbols\n dead scan failed: boom\n").deadNote).toBe(
      "dead scan failed: boom",
    );
  });
});

describe("hosts", () => {
  const v = parseHosts(hostsText);

  test("splits blocks by host variant with notes, modules and the skip line", () => {
    expect(v.blocks.map((b) => [b.kind, b.name, b.note])).toEqual([
      ["CLI", "Claude Code", ""],
      ["CLI", "Codex", ""],
      ["Desktop", "Cursor", ""],
      ["CLI", "Gemini CLI", "not found"],
      ["Desktop", "Zed", "not installed"],
    ]);
    expect(v.blocks[0]).toMatchObject({
      app: "/opt/homebrew/bin/claude (2.1.4)",
      config: ["~/.claude/settings.json", "~/.claude.json"],
    });
    expect(v.blocks[0]?.modules[3]).toEqual(["plugin", "not installed --plugin"]);
    expect(v.blocks[3]?.app).toBeNull();
    expect(v.blocks[4]?.skip).toContain("rtok agents install zed");
  });

  test("the first probe says it is still running", () => {
    expect(parseHosts("probing hosts…\n")).toEqual({ probing: true, blocks: [], other: [] });
  });

  test("a line outside any block is kept", () => {
    expect(parseHosts("stray line\nCLI: X\n").other).toEqual(["stray line"]);
  });
});

describe("config", () => {
  const all = parseConfig(configText);

  test("one entry per `key = value (source)` line", () => {
    expect(all).toHaveLength(12);
    expect(all[1]).toEqual({ key: "core.db_path", value: '"~/.rtok/rtok.db"', source: "default" });
    expect(all.find((e) => e.key === "web.port")?.source).toBe("flag");
  });

  test("filters by layer and by key or value text", () => {
    const user = all.filter((e) => matchesConfig(e, "user", ""));
    expect(user.map((e) => e.key)).toEqual(["core.retain_calls_days", "demon.services"]);
    expect(all.filter((e) => matchesConfig(e, "all", "COMPRESS")).map((e) => e.key)).toEqual([
      "proxy.mode",
    ]);
  });

  test("groups by table in first-seen order", () => {
    expect(groupConfig(all).map(([g, rows]) => [g, rows.length])).toEqual([
      ["core", 3],
      ["log", 2],
      ["hook", 1],
      ["mcp", 1],
      ["proxy", 2],
      ["web", 1],
      ["demon", 1],
      ["plugins", 1],
    ]);
  });
});

describe("services", () => {
  const v = parseServices(servicesText);

  test("reads running and stopped services", () => {
    expect(v.services).toEqual([
      {
        name: "proxy",
        running: true,
        pid: "48211",
        uptimeSecs: 18_342,
        log: "~/.rtok/demon/proxy.log",
      },
      { name: "mcp", running: false, pid: null, uptimeSecs: null, log: "~/.rtok/demon/mcp.log" },
      {
        name: "web",
        running: true,
        pid: "48230",
        uptimeSecs: 18_339,
        log: "~/.rtok/demon/web.log",
      },
    ]);
  });

  test("reads the exporter line and the last flush", () => {
    expect(v.otel).toEqual({
      endpoint: "http://127.0.0.1:4318",
      callsMark: 5102,
      callsPending: 18,
      logsMark: 2210,
      logsPending: 0,
      sessionsMark: 311,
    });
    expect(v.lastFlush).toContain("otel exported 120 calls");
    expect(parseServices("proxy  stopped  pid=-  uptime=-  log=/l\n").otel).toBeNull();
  });
});

describe("worktrees", () => {
  const v = parseWorktrees(worktreesText);

  test("maps the header to fields, including the two agent columns", () => {
    if (v.kind !== "table") throw new Error("expected a table");
    expect(v.rows).toHaveLength(5);
    expect(v.rows[1]).toMatchObject({
      path: "~/GitHub/listepo/_worktrees/rtok-t307",
      branch: "t307-report-sparkline",
      owner: "Claude Code / sonnet-5",
      agent: "1a2b3c4d claude",
      agentState: "active",
      state: "dirty",
      seen: "3m",
      source: "41 MB",
      cache: "2.8 GB",
    });
    expect(v.rows[3]?.agent).toBe("seen codex 9f8e7d6c");
    expect(v.total).toBe("5 worktrees: 167 MB source, 12.1 GB build cache");
    expect(v.note).toContain("logical bytes");
  });

  test("filters by state and text", () => {
    if (v.kind !== "table") throw new Error("expected a table");
    expect(v.rows.filter((r) => matchesWorktree(r, "dirty", "")).length).toBe(1);
    expect(v.rows.filter((r) => matchesWorktree(r, "all", "CLAUDE")).length).toBe(1);
  });

  test("ordinary non-table states are messages", () => {
    expect(parseWorktrees("reading worktrees…\n")).toEqual({
      kind: "message",
      message: "reading worktrees…",
    });
    expect(parseWorktrees("not a git repository\n").kind).toBe("message");
  });
});
