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

export function stateOf(p: ProjectRow) {
  return { label: p.state, ...(STATES[p.state] ?? { tone: "muted" as const, hint: "" }) };
}

export function filterProjects(rows: ProjectRow[], query: string): ProjectRow[] {
  const q = query.trim().toLowerCase();
  return q ? rows.filter((p) => `${p.name} ${p.root}`.toLowerCase().includes(q)) : rows;
}

/** Projects `from` does not link to yet, and that can still be linked (a missing root cannot). */
export function linkTargets(rows: ProjectRow[], from: ProjectRow): ProjectRow[] {
  const linked = new Set(from.links.map((l) => l.to));
  return rows.filter((p) => p.id !== from.id && !p.missing && !linked.has(p.id));
}

/** What the server does with a registry request, for the sample server and the tests. */
export function applyProject(rows: ProjectRow[], req: ProjectRequest): ProjectRow[] {
  if (req.action === "select") {
    if (!rows.some((p) => String(p.id) === req.project)) return rows;
    return rows.map((p) => ({ ...p, selected: String(p.id) === req.project }));
  }
  const pairs = [[req.from, req.to]];
  if (req.both) pairs.push([req.to, req.from]);
  return pairs.reduce(
    (acc, [from, to]) => acc.map((p) => relink(acc, p, from!, to!, req.action)),
    rows,
  );
}

function relink(
  rows: ProjectRow[],
  p: ProjectRow,
  from: string,
  to: string,
  action: "link" | "unlink",
) {
  if (String(p.id) !== from) return p;
  const target = rows.find((t) => String(t.id) === to);
  const rest = p.links.filter((l) => String(l.to) !== to);
  if (action === "unlink" || !target) return { ...p, links: rest };
  return {
    ...p,
    links: [...rest, { kind: "manual" as const, name: target.name, reason: null, to: target.id }],
  };
}
