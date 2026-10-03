// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { connectionKey, createApi, messageKey, snapshotKey, type Api } from "./query";
import { connectSample, isSampleRequested, sampleSnapshot } from "./sample";
import type { Snapshot } from "./snapshot.gen";
import type { Connect, Connection, Handlers } from "./ws";

// A scripted connection: the test plays the server by calling the captured handlers.
function scripted() {
  let handlers!: Handlers;
  const sent: unknown[] = [];
  let open = true;
  const connection: Connection = {
    send: (m) => (open ? (sent.push(m), true) : false),
    close: () => handlers.onState("closed"),
  };
  const connect: Connect = (h) => ((handlers = h), connection);
  return { connect, sent, server: () => handlers, setOpen: (v: boolean) => (open = v) };
}

let queryClient: QueryClient;
beforeEach(() => {
  queryClient = new QueryClient();
});
afterEach(() => vi.useRealTimers());

describe("cache wiring", () => {
  test("each snapshot replaces the cached one and connection state is tracked", () => {
    const s = scripted();
    createApi(queryClient, s.connect).open();
    s.server().onState("open");
    s.server().onFrame({ type: "snapshot", snapshot: sampleSnapshot });
    expect(queryClient.getQueryData(connectionKey)).toBe("open");
    expect(queryClient.getQueryData(snapshotKey)).toEqual(sampleSnapshot);

    const next: Snapshot = { ...sampleSnapshot, logs: ["later"] };
    s.server().onFrame({ type: "snapshot", snapshot: next });
    expect(queryClient.getQueryData<Snapshot>(snapshotKey)?.logs).toEqual(["later"]);
  });

  test("a failed tick keeps the last pages and sets the error; the next good frame clears it", () => {
    const s = scripted();
    createApi(queryClient, s.connect).open();
    s.server().onFrame({ type: "snapshot_error", error: "early" });
    expect(queryClient.getQueryData(snapshotKey)).toBeUndefined();

    s.server().onFrame({ type: "snapshot", snapshot: sampleSnapshot });
    s.server().onFrame({ type: "snapshot_error", error: "boom" });
    expect(queryClient.getQueryData<Snapshot>(snapshotKey)).toMatchObject({
      error: "boom",
      logs: sampleSnapshot.logs,
    });
    s.server().onFrame({ type: "snapshot", snapshot: sampleSnapshot });
    expect(queryClient.getQueryData<Snapshot>(snapshotKey)?.error).toBeUndefined();
  });

  test("open is idempotent and close shuts the connection", () => {
    const connect = vi.fn(scripted().connect);
    const api = createApi(queryClient, connect);
    api.open();
    api.open();
    expect(connect).toHaveBeenCalledTimes(1);
    api.close();
    expect(queryClient.getQueryData(connectionKey)).toBe("closed");
  });
});

describe("set mutation", () => {
  test("sends the key and value; fails when not connected", async () => {
    const s = scripted();
    const api = createApi(queryClient, s.connect);
    await expect(api.set({ key: "plugins.a.enabled", value: true })).rejects.toThrow(
      "not connected",
    );
    api.open();
    await api.set({ key: "plugins.a.enabled", value: true });
    expect(s.sent).toEqual([{ set: { key: "plugins.a.enabled", value: true } }]);
    s.setOpen(false);
    await expect(api.set({ key: "plugins.a.enabled", value: false })).rejects.toThrow(
      "not connected",
    );
  });

  test("a refusal is stored as the server message", () => {
    const s = scripted();
    createApi(queryClient, s.connect).open();
    s.server().onFrame({ type: "message", text: "refused key x" });
    expect(queryClient.getQueryData(messageKey)).toBe("refused key x");
  });
});

describe("expand mutation", () => {
  let s: ReturnType<typeof scripted>;
  let api: Api;
  beforeEach(() => {
    vi.useFakeTimers();
    s = scripted();
    api = createApi(queryClient, s.connect, 1000);
    api.open();
  });

  test("resolves on the frame with the matching id only", async () => {
    const p = api.expand("a1");
    expect(s.sent).toEqual([{ expand: "a1" }]);
    s.server().onFrame({ type: "expand", id: "other", text: "wrong" });
    s.server().onFrame({ type: "expand", id: "a1", text: "body" });
    await expect(p).resolves.toBe("body");
  });

  test("rejects on a server message", async () => {
    const p = api.expand("nope");
    s.server().onFrame({ type: "message", text: "unknown archive id: nope" });
    await expect(p).rejects.toThrow("unknown archive id: nope");
  });

  test("rejects on timeout, and a late reply is ignored", async () => {
    const p = api.expand("a1");
    const assertion = expect(p).rejects.toThrow("timed out");
    vi.advanceTimersByTime(1000);
    await assertion;
    expect(() => s.server().onFrame({ type: "expand", id: "a1", text: "late" })).not.toThrow();
  });

  test("rejects when the connection drops or was never open", async () => {
    const dropped = api.expand("a1");
    s.server().onState("closed");
    await expect(dropped).rejects.toThrow("connection closed");

    s.setOpen(false);
    await expect(api.expand("a2")).rejects.toThrow("not connected");
  });
});

describe("?sample source", () => {
  test("is selected by the query string", () => {
    expect(isSampleRequested("?sample")).toBe(true);
    expect(isSampleRequested("?x=1&sample=1")).toBe(true);
    expect(isSampleRequested("")).toBe(false);
  });

  test("serves the fixture offline, flips a plugin on set and answers expand", async () => {
    const api = createApi(queryClient, connectSample);
    api.open();
    await Promise.resolve();
    expect(queryClient.getQueryData(connectionKey)).toBe("open");
    expect(queryClient.getQueryData<Snapshot>(snapshotKey)?.plugins.map((p) => p.enabled)).toEqual([
      true,
      false,
      true,
      true,
      true,
      false,
    ]);

    const expanded = api.expand("abc");
    await api.set({ key: "plugins.read.enabled", value: true });
    await expect(expanded).resolves.toContain("abc");
    await Promise.resolve();
    expect(queryClient.getQueryData<Snapshot>(snapshotKey)?.plugins.map((p) => p.enabled)).toEqual([
      true,
      true,
      true,
      true,
      true,
      false,
    ]);
    expect(sampleSnapshot.plugins[1]?.enabled).toBe(false);
  });

  test("refuses a key outside the plugin allowlist", async () => {
    const api = createApi(queryClient, connectSample);
    api.open();
    await api.set({ key: "proxy.enabled", value: true });
    await Promise.resolve();
    expect(queryClient.getQueryData(messageKey)).toBe("refused key proxy.enabled");
  });
});

describe("doctor requests", () => {
  const selection = { keep: [], toggled: [{ source: "/p", path: "hooks.Stop[0]" }] };
  const plan = { items: [], diff: "", refused: [] };

  test("a plan and an apply each wait for their own frame, in the order asked", async () => {
    const s = scripted();
    const api = createApi(queryClient, s.connect);
    api.open();
    const planned = api.doctorPlan(selection);
    const applied = api.doctorApply(selection);
    expect(s.sent).toEqual([
      { doctor: { action: "plan", selection } },
      { doctor: { action: "apply", selection } },
    ]);
    s.server().onFrame({ type: "doctorfixed", fixed: { text: "done", code: 0 } });
    s.server().onFrame({ type: "doctorplan", plan });
    await expect(planned).resolves.toEqual(plan);
    await expect(applied).resolves.toEqual({ text: "done", code: 0 });
  });

  test("a refusal, a closed link and a missing link fail the request", async () => {
    const s = scripted();
    const api = createApi(queryClient, s.connect);
    api.open();
    const refused = api.doctorPlan(selection);
    s.server().onFrame({ type: "message", text: "doctor needs an action and a selection" });
    await expect(refused).rejects.toThrow("doctor needs");
    const dropped = api.doctorApply(selection);
    s.server().onState("closed");
    await expect(dropped).rejects.toThrow("connection closed");
    s.setOpen(false);
    await expect(api.doctorPlan(selection)).rejects.toThrow("not connected");
  });

  test("the sample machine plans with the terminal defaults and applies the selection", async () => {
    const api = createApi(queryClient, connectSample);
    api.open();
    const first = await api.doctorPlan({ keep: [], toggled: [] });
    expect(first.items.map((i) => [i.shared, i.selected])).toEqual([
      [false, true],
      [true, false],
      [false, true],
    ]);
    expect(first.diff).not.toContain("/work/app");
    const flipped = await api.doctorPlan({
      keep: [],
      toggled: [{ source: "/work/app/.claude/settings.json", path: "hooks.Stop[0].hooks[0]" }],
    });
    expect(flipped.diff).toContain("/work/app");
    const done = await api.doctorApply({ keep: [], toggled: [] });
    expect(done).toMatchObject({ code: 0 });
    expect(done.text).toContain("2 entries removed");
  });
});
