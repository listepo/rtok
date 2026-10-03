// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { describe, expect, test } from "vitest";
import { project } from "../api/sampleRows";
import { applyProject, filterProjects, linkTargets, stateOf } from "./projectLogic";

const rows = () => [project(1, "rtok", { selected: true }), project(2, "ketch"), project(3, "web")];
const linksOf = (r: ReturnType<typeof rows>, id: number) =>
  r.find((p) => p.id === id)!.links.map((l) => l.to);

describe("project reducers", () => {
  test("select moves the selection to one project", () => {
    const next = applyProject(rows(), { action: "select", project: "3" });
    expect(next.map((p) => p.selected)).toEqual([false, false, true]);
  });

  test("an unknown project changes nothing, as the server refuses it", () => {
    const next = applyProject(rows(), { action: "select", project: "99" });
    expect(next).toEqual(rows());
  });

  test("link adds a manual link, once, and both ways when asked", () => {
    let next = applyProject(rows(), { action: "link", from: "1", to: "2", both: false });
    next = applyProject(next, { action: "link", from: "1", to: "2", both: true });
    expect(linksOf(next, 1)).toEqual([2]);
    expect(linksOf(next, 2)).toEqual([1]);
    expect(next[0]!.links[0]).toMatchObject({ kind: "manual", name: "ketch" });
  });

  test("unlink removes only the asked direction", () => {
    const both = applyProject(rows(), { action: "link", from: "1", to: "2", both: true });
    const next = applyProject(both, { action: "unlink", from: "1", to: "2", both: false });
    expect(linksOf(next, 1)).toEqual([]);
    expect(linksOf(next, 2)).toEqual([1]);
  });

  test("an unknown target changes nothing", () => {
    const next = applyProject(rows(), { action: "link", from: "1", to: "99", both: false });
    expect(linksOf(next, 1)).toEqual([]);
  });
});

describe("project view logic", () => {
  test("filter matches the name or the root, case-insensitively", () => {
    expect(filterProjects(rows(), " KET ").map((p) => p.name)).toEqual(["ketch"]);
    expect(filterProjects(rows(), "/work/w").map((p) => p.name)).toEqual(["web"]);
    expect(filterProjects(rows(), "")).toHaveLength(3);
  });

  test("link targets skip itself, linked and missing projects", () => {
    const r = [...rows(), project(4, "gone", { missing: true, state: "missing", index: null })];
    const linked = applyProject(r, { action: "link", from: "1", to: "2", both: false });
    expect(linkTargets(linked, linked[0]!).map((p) => p.name)).toEqual(["web"]);
  });

  test("every state has a tone; an unknown one is muted", () => {
    const tones = ["ok", "not indexed", "indexing", "stale", "failed", "missing"].map(
      (state) => stateOf(project(1, "x", { state })).tone,
    );
    expect(tones).toEqual(["ok", "muted", "info", "warn", "fail", "fail"]);
    expect(stateOf(project(1, "x", { state: "odd" })).tone).toBe("muted");
  });
});
