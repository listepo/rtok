// A fuller snapshot than `sampleSnapshot` for page stories and tests: several plugins in
// every state, calls on all three surfaces including a failure, and an archive handle.
import { sampleSnapshot } from "../api/sample";
import type { CallRow, PluginPage, Snapshot } from "../api/snapshot.gen";

const plugin = (id: string, over: Partial<PluginPage> = {}): PluginPage => ({
  enabled: true,
  fields: [],
  id,
  saves_tokens: true,
  stats: null,
  summary: `${id} plugin`,
  surfaces: ["hook"],
  title: id[0]?.toUpperCase() + id.slice(1),
  ...over,
});

const stats = (before: number, after: number, rows: number) => ({
  cache_create: 0,
  cache_read: 0,
  est_before: before,
  est_after: after,
  input: before,
  output: after,
  rows,
});

const call = (id: number, over: Partial<CallRow>): CallRow => ({
  api: null,
  cache_create: null,
  cache_read: null,
  error: null,
  host: "claude-code",
  id,
  input: null,
  kind: "hook",
  model: null,
  ms: 5,
  name: "PostToolUse",
  ok: 1,
  output: null,
  parent_id: null,
  plugin: "shell",
  provider: null,
  session: "3f9a2c1e-0000",
  surface: "hook",
  ts: 1_790_000_000 + id * 30,
  ...over,
});

export const richSnapshot: Snapshot = {
  ...sampleSnapshot,
  plugins: [
    plugin("shell", {
      stats: stats(120_000, 30_000, 140),
      fields: [["mode", "aggressive"]],
      summary: "Shrinks noisy shell output.",
    }),
    plugin("read", {
      surfaces: ["mcp", "hook"],
      stats: stats(60_000, 40_000, 55),
      summary: "Reads files through the archive.",
    }),
    plugin("graph", { enabled: false, surfaces: ["mcp"], summary: "Code graph." }),
    plugin("ledger", { saves_tokens: false, surfaces: ["proxy"], summary: "Usage ledger." }),
  ],
  calls: [
    call(8, {
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
    }),
    call(7, { surface: "mcp", kind: "mcp", name: "read", plugin: "read", ms: 12 }),
    call(6, {
      name: "PreToolUse",
      ok: 0,
      error: "plugin shell panicked: index not built",
      ms: 3_400,
    }),
    call(5, {}),
    call(4, { surface: "mcp", kind: "mcp", name: "search", plugin: "read", ms: 41 }),
    call(3, {}),
    call(2, { name: "SessionStart", plugin: null, ms: 2 }),
    call(1, { surface: "mcp", kind: "mcp", name: "outline", plugin: "read", ms: 9 }),
  ],
  ref_ids: { "7": "arch-7f3a", "5": "arch-5b21" },
  usage: {
    ...sampleSnapshot.usage,
    alerts: ["context window 91% full in session 3f9a2c1e"],
    turns: [4_000, 6_500, 9_000, 8_000, 12_000, 15_000, 11_000],
  },
  sessions: [
    ...sampleSnapshot.sessions,
    {
      ...(sampleSnapshot.sessions[0] as Snapshot["sessions"][number]),
      id: "old",
      ended_at: 1_790_000_050,
      host: "codex",
    },
  ],
};
