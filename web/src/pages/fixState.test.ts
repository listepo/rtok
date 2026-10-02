// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { describe, expect, test } from "vitest";
import type { Item, Plan } from "../api/snapshot.gen";
import { fixReducer, initialFix, selectedCount, type FixAction, type FixState } from "./fixState";

const ref = (source: string, path = "hooks.Stop[0]") => ({ source, path });

const item = (source: string, selected: boolean): Item => ({
  source,
  path: "hooks.Stop[0]",
  kind: "broken-hook",
  agent: "claude",
  label: "/gone.sh",
  detail: "missing",
  kept_in: null,
  shared: !selected,
  selected,
  can_keep: false,
});

const plan = (...items: Item[]): Plan => ({ items, diff: "", refused: [] });

const run = (start: FixState, ...actions: FixAction[]) => actions.reduce(fixReducer, start);

describe("doctor fix reducer", () => {
  test("a toggle is a delta from the defaults and a second toggle undoes it", () => {
    const on = run(initialFix, { type: "toggle", ref: ref("/p") });
    expect(on.selection.toggled).toEqual([ref("/p")]);
    expect(on.phase).toBe("loading");
    const off = fixReducer(on, { type: "toggle", ref: ref("/p") });
    expect(off.selection.toggled).toEqual([]);
    expect(off.seq).toBe(2);
  });

  test("a plan for an older selection is dropped", () => {
    const s = run(initialFix, { type: "toggle", ref: ref("/p") });
    const late = fixReducer(s, { type: "planned", seq: 0, plan: plan(item("/u", true)) });
    expect(late).toBe(s);
    const fresh = fixReducer(s, { type: "planned", seq: s.seq, plan: plan(item("/u", true)) });
    expect(fresh.phase).toBe("ready");
    expect(selectedCount(fresh.plan)).toBe(1);
  });

  test("a keep swap starts the selection over from the new defaults", () => {
    const s = run(initialFix, { type: "toggle", ref: ref("/p") }, { type: "keep", ref: ref("/u") });
    expect(s.selection).toEqual({ keep: [ref("/u")], toggled: [] });
  });

  test("confirming needs something selected and a change withdraws it", () => {
    const none = run(initialFix, { type: "planned", seq: 0, plan: plan(item("/p", false)) });
    expect(fixReducer(none, { type: "ask" }).phase).toBe("ready");
    const ready = run(initialFix, { type: "planned", seq: 0, plan: plan(item("/u", true)) });
    const asked = fixReducer(ready, { type: "ask" });
    expect(asked.phase).toBe("confirming");
    expect(fixReducer(asked, { type: "cancel" }).phase).toBe("ready");
    expect(fixReducer(asked, { type: "toggle", ref: ref("/u") }).phase).toBe("loading");
  });

  test("only a confirmed request is applied, and a finished one ignores edits", () => {
    const ready = run(initialFix, { type: "planned", seq: 0, plan: plan(item("/u", true)) });
    expect(fixReducer(ready, { type: "applying" }).phase).toBe("ready");
    const applying = run(ready, { type: "ask" }, { type: "applying" });
    expect(applying.phase).toBe("applying");
    expect(fixReducer(applying, { type: "toggle", ref: ref("/u") })).toBe(applying);
    const done = fixReducer(applying, { type: "applied", fixed: { text: "ok", code: 0 } });
    expect(done.phase).toBe("done");
    expect(fixReducer(done, { type: "keep", ref: ref("/u") })).toBe(done);
    expect(fixReducer(done, { type: "again" })).toMatchObject({ phase: "loading", plan: null });
  });

  test("a failed request shows the error and keeps the selection", () => {
    const s = run(
      initialFix,
      { type: "toggle", ref: ref("/p") },
      { type: "failed", error: "connection closed" },
    );
    expect(s).toMatchObject({ phase: "error", error: "connection closed" });
    expect(s.selection.toggled).toEqual([ref("/p")]);
  });
});
