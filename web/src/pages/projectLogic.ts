// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

import type { ProjectRequest, ProjectRow } from "../api/snapshot.gen";
import type { PillTone } from "../ui/Pill";

/** Above this many projects the selector gets a search box. */
export const SEARCH_ABOVE = 10;

/** The index states the indicator draws; `indexing` and `failed` arrive once the server reports them. */
const STATES: Record<string, { tone: PillTone; hint: string }> = {
  ok: { tone: "ok", hint: "index is fresh" },
  "not indexed": { tone: "muted", hint: "nothing indexed yet" },
  indexing: { tone: "info", hint: "index is being built" },
  stale: { tone: "warn", hint: "files changed since the last index" },
  failed: { tone: "fail", hint: "the last index run failed" },
  missing: { tone: "fail", hint: "the directory no longer exists" },
};

export function stateOf(p: { state: string }) {
  return { label: p.state, ...(STATES[p.state] ?? { tone: "muted" as const, hint: "" }) };
}

export function filterProjects(rows: ProjectRow[], query: string): ProjectRow[] {
  const q = query.trim().toLowerCase();
  return q ? rows.filter((p) => `${p.name} ${p.root}`.toLowerCase().includes(q)) : rows;
}

/** What the server does with a registry request, for the sample server and the tests. */
export function applyProject(rows: ProjectRow[], req: ProjectRequest): ProjectRow[] {
  if (!rows.some((p) => String(p.id) === req.project)) return rows;
  return rows.map((p) => ({ ...p, selected: String(p.id) === req.project }));
}
