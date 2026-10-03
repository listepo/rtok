// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import { describe, expect, test } from "vitest";
import { project } from "../api/sampleRows";
import { applyProject, filterProjects, stateOf } from "./projectLogic";

const rows = () => [project(1, "rtok", { selected: true }), project(2, "ketch"), project(3, "web")];

describe("project logic", () => {
  test("select moves the selection to one project", () => {
    const next = applyProject(rows(), { action: "select", project: "3" });
    expect(next.map((p) => p.selected)).toEqual([false, false, true]);
  });

  test("an unknown project changes nothing, as the server refuses it", () => {
    const next = applyProject(rows(), { action: "select", project: "99" });
    expect(next).toEqual(rows());
  });

  test("filter matches the name or the root, case-insensitively", () => {
    expect(filterProjects(rows(), " KET ").map((p) => p.name)).toEqual(["ketch"]);
    expect(filterProjects(rows(), "/work/w").map((p) => p.name)).toEqual(["web"]);
    expect(filterProjects(rows(), "")).toHaveLength(3);
  });

  test("every state has a tone; an unknown one is muted", () => {
    const tones = ["ok", "not indexed", "indexing", "stale", "failed", "missing"].map(
      (state) => stateOf(project(1, "x", { state })).tone,
    );
    expect(tones).toEqual(["ok", "muted", "info", "warn", "fail", "fail"]);
    expect(stateOf(project(1, "x", { state: "odd" })).tone).toBe("muted");
  });
});
