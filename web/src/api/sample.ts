// Offline data source (`?sample`) and the snapshot fixture shared by Storybook, Vitest and
// offline e2e. Typed as `Snapshot`, so a schema change breaks `tsc` here instead of drifting.
import type { Connect, Connection } from "./ws";
import type { Snapshot } from "./snapshot.gen";

const stats = {
  cache_create: 1_200,
  cache_read: 48_000,
  est_after: 9_000,
  est_before: 31_000,
  input: 5_400,
  output: 2_100,
  rows: 42,
};

export const sampleSnapshot: Snapshot = {
  type: "snapshot",
  calls: [
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
      session: "sample-session",
      surface: "hook",
      ts: 1_790_000_100,
    },
    {
      api: null,
      cache_create: null,
      cache_read: null,
      error: null,
      host: "claude-code",
      id: 1,
      input: null,
      kind: "mcp",
      model: null,
      ms: 12,
      name: "read",
      ok: 1,
      output: null,
      parent_id: null,
      plugin: "read",
      provider: null,
      session: "sample-session",
      surface: "mcp",
      ts: 1_790_000_000,
    },
  ],
  config: "plugins.shell.enabled = true",
  doctor: null,
  graph: null,
  hosts: "claude-code  1.0.0  hook, mcp",
  logs: ["rtok hook PostToolUse ok 4 ms"],
  plugins: [
    {
      enabled: true,
      fields: [],
      id: "shell",
      saves_tokens: true,
      stats,
      summary: "Shrinks noisy shell output.",
      surfaces: ["hook"],
      title: "Shell",
    },
    {
      enabled: false,
      fields: [],
      id: "read",
      saves_tokens: true,
      stats: null,
      summary: "Reads files through the archive.",
      surfaces: ["mcp"],
      title: "Read",
    },
  ],
  ref_ids: { "2": "sample-archive-id" },
  services: null,
  sessions: [
    {
      api: "anthropic",
      cache_create: 1_200,
      cache_read: 48_000,
      ended_at: null,
      host: "claude-code",
      id: "sample-session",
      input: 5_400,
      last_activity: 1_790_000_100,
      model: "claude-sonnet-5-5",
      output: 2_100,
      project: "rtok",
      provider: "anthropic",
      started_at: 1_789_999_000,
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
    turns: [4_000, 6_500, 9_000],
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
