// A fuller snapshot than `sampleSnapshot` for page stories and tests: several plugins in
// every state, calls on all three surfaces including a failure, and an archive handle.
import { sampleSnapshot } from "../api/sample";
import { call as baseCall, plugin, stats } from "../api/sampleRows";
import type { CallRow, Snapshot } from "../api/snapshot.gen";

// The tests and stories below quote this session id.
const call = (id: number, over: Partial<CallRow>): CallRow =>
  baseCall(id, { session: "3f9a2c1e-0000", ...over });

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
    ...sampleSnapshot.sessions.filter((s) => s.id === "sample-session"),
    {
      ...(sampleSnapshot.sessions[1] as Snapshot["sessions"][number]),
      id: "old",
      ended_at: 1_790_000_050,
      host: "codex",
    },
  ],
};
