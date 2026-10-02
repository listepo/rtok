// Offline data source (`?sample`) and the snapshot fixture shared by Storybook, Vitest and
// offline e2e. Typed as `Snapshot`, so a schema change breaks `tsc` here instead of drifting.
import type { Connect, Connection } from "./ws";
import { call, plugin, stats } from "./sampleRows";
import type { Report, Snapshot } from "./snapshot.gen";

const shellStats = {
  cache_create: 1_200,
  cache_read: 48_000,
  est_after: 9_000,
  est_before: 31_000,
  input: 5_400,
  output: 2_100,
  rows: 42,
};

// Enough variety for every overview block: a warning, a failing check and a skipped one.
const doctor: Report = {
  agents: [
    {
      host: "claude-code",
      kind: "cli",
      modules: [
        { name: "hooks", note: "", state: "installed" },
        { name: "mcp", note: "", state: "installed" },
        { name: "proxy", note: "rtok agent setup --proxy", state: "not_installed" },
      ],
    },
  ],
  auto_compact_window: null,
  bash_max_output_length: "30000",
  hooks_by_event: { PostToolUse: 1, PreToolUse: 1, SessionStart: 1 },
  hooks_total: 3,
  instructions: {
    duplicates: [],
    rows: [
      { name: "CLAUDE.md", path: "CLAUDE.md", tokens: 1_450, warn: false },
      { name: "AGENTS.md", path: "apps/rtok/AGENTS.md", tokens: 5_200, warn: true },
    ],
  },
  mcp: [{ cmd: "rtok mcp", desc_tokens: 640, name: "rtok", tools: 9 }],
  mcp_tool_search_disabled: true,
  overlaps: ["host-native grep duplicates the rtok search tool ([mcp] search = false)"],
  proxy: "claude-code → rtok proxy → api.anthropic.com",
  proxy_openai: "",
};

const session = "sample-session";

export const sampleSnapshot: Snapshot = {
  type: "snapshot",
  calls: [
    call(14, {
      surface: "proxy",
      kind: "request",
      name: "messages",
      plugin: null,
      ms: 1_820,
      model: "claude-sonnet-5-5",
      input: 900,
      cache_read: 40_000,
      output: 300,
      api: "anthropic",
      provider: "anthropic",
      ts: 1_790_000_620,
    }),
    call(13, {
      surface: "mcp",
      kind: "mcp",
      name: "symbol",
      plugin: "graph",
      ms: 31,
      ts: 1_790_000_580,
    }),
    call(12, { ts: 1_790_000_540 }),
    call(11, {
      name: "PreToolUse",
      ok: 0,
      error: "plugin shell panicked: index not built",
      ms: 3_400,
      ts: 1_790_000_500,
    }),
    call(10, {
      surface: "mcp",
      kind: "mcp",
      name: "read",
      plugin: "read",
      ms: 12,
      ts: 1_790_000_460,
    }),
    call(9, { ts: 1_790_000_420 }),
    call(8, {
      surface: "proxy",
      kind: "request",
      name: "messages",
      plugin: null,
      ms: 940,
      model: "claude-sonnet-5-5",
      input: 700,
      cache_read: 31_000,
      output: 210,
      api: "anthropic",
      provider: "anthropic",
      ts: 1_790_000_380,
    }),
    call(7, {
      surface: "mcp",
      kind: "mcp",
      name: "search",
      plugin: "graph",
      ms: 41,
      ts: 1_790_000_300,
    }),
    call(6, { ts: 1_790_000_260 }),
    call(5, { name: "SessionStart", plugin: null, ms: 2, ts: 1_790_000_220 }),
    call(4, {
      surface: "mcp",
      kind: "mcp",
      name: "outline",
      plugin: "read",
      ms: 9,
      ts: 1_790_000_180,
    }),
    call(3, { ts: 1_790_000_140 }),
    {
      api: "anthropic",
      cache_create: 0,
      cache_read: 12_000,
      error: null,
      host: "claude-code",
      id: 2,
      input: 800,
      kind: "hook",
      model: "claude-sonnet-5-5",
      ms: 4,
      name: "PostToolUse",
      ok: 1,
      output: 120,
      parent_id: null,
      plugin: "shell",
      provider: "anthropic",
      session,
      surface: "hook",
      ts: 1_790_000_100,
    },
    call(1, {
      surface: "mcp",
      kind: "mcp",
      name: "read",
      plugin: "read",
      ms: 12,
      ts: 1_790_000_000,
    }),
  ],
  config: "plugins.shell.enabled = true",
  doctor,
  graph: null,
  hosts: "claude-code  1.0.0  hook, mcp",
  logs: [
    "rtok hook PostToolUse ok 4 ms",
    "2026-10-02 12:03:20 ERROR hook/PreToolUse: plugin shell panicked: index not built",
    "2026-10-02 12:02:40 WARN proxy/messages: slow upstream (1820 ms)",
    "2026-10-02 12:01:05 INFO mcp/read: archived 4 files",
  ],
  plugins: [
    plugin("shell", {
      stats: shellStats,
      summary: "Shrinks noisy shell output.",
      title: "Shell",
    }),
    plugin("read", {
      enabled: false,
      surfaces: ["mcp"],
      summary: "Reads files through the archive.",
      title: "Read",
    }),
    plugin("graph", {
      stats: stats(48_000, 21_000, 19),
      surfaces: ["mcp"],
      summary: "Code graph: symbols, callers and impact.",
    }),
    plugin("archive", {
      stats: stats(14_000, 9_500, 7),
      surfaces: ["hook", "mcp"],
      summary: "Lossless archive behind expand.",
    }),
    plugin("ledger", { saves_tokens: false, surfaces: ["proxy"], summary: "Usage ledger." }),
    plugin("budget", { enabled: false, surfaces: ["proxy"], summary: "Injection budget." }),
  ],
  ref_ids: { "2": "sample-archive-id", "10": "sample-archive-read", "13": "sample-archive-graph" },
  services: null,
  sessions: [
    {
      api: "openai",
      cache_create: 800,
      cache_read: 21_000,
      ended_at: null,
      host: "codex",
      id: "9b2e4d1a-sample",
      input: 2_900,
      last_activity: 1_790_000_620,
      model: "gpt-5.5",
      output: 640,
      project: "ketch",
      provider: "openai",
      started_at: 1_790_000_100,
    },
    {
      api: "anthropic",
      cache_create: 1_200,
      cache_read: 48_000,
      ended_at: null,
      host: "claude-code",
      id: session,
      input: 5_400,
      last_activity: 1_790_000_100,
      model: "claude-sonnet-5-5",
      output: 2_100,
      project: "rtok",
      provider: "anthropic",
      started_at: 1_789_999_000,
    },
    {
      api: "anthropic",
      cache_create: 300,
      cache_read: 9_000,
      ended_at: 1_789_999_900,
      host: "claude-code",
      id: "41c7f0aa-sample",
      input: 1_100,
      last_activity: 1_789_999_900,
      model: "claude-sonnet-5-5",
      output: 400,
      project: "rtok",
      provider: "anthropic",
      started_at: 1_789_999_200,
    },
  ],
  skills: { header: "0 skills listed", rows: [] },
  stats: null,
  usage: {
    cache_create: 1_200,
    cache_read: 48_000,
    ctt: 90_000,
    est_after: 9_000,
    est_before: 31_000,
    input: 5_400,
    output: 2_100,
    rows: 42,
    turns: [4_000, 6_500, 9_000, 7_200, 11_000, 14_000, 12_500, 16_000],
  },
  worktrees: null,
};

export const isSampleRequested = (search: string): boolean =>
  new URLSearchParams(search).has("sample");

export const connectSample: Connect = (handlers) => {
  let snapshot = structuredClone(sampleSnapshot);
  let stopped = false;
  // Frames land on a microtask so callers can register a reply handler after `send`.
  const later = (fn: () => void) =>
    queueMicrotask(() => {
      if (!stopped) fn();
    });
  const emit = () => handlers.onFrame({ type: "snapshot", snapshot: structuredClone(snapshot) });

  handlers.onState("connecting");
  later(() => {
    handlers.onState("open");
    emit();
  });

  const connection: Connection = {
    send(message) {
      if (stopped) return false;
      if ("expand" in message) {
        const { expand: id } = message;
        later(() => handlers.onFrame({ type: "expand", id, text: `sample payload for ${id}` }));
        return true;
      }
      const { key, value } = message.set;
      const id = /^plugins\.([^.]+)\.enabled$/.exec(key)?.[1];
      const plugin = snapshot.plugins.find((p) => p.id === id);
      if (!plugin) {
        later(() => handlers.onFrame({ type: "message", text: `refused key ${key}` }));
        return true;
      }
      snapshot = {
        ...snapshot,
        plugins: snapshot.plugins.map((p) => (p === plugin ? { ...p, enabled: value } : p)),
      };
      later(emit);
      return true;
    },
    close() {
      stopped = true;
      handlers.onState("closed");
    },
  };
  return connection;
};
