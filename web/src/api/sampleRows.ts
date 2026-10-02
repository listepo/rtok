// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Row builders for sample data, shared by `?sample` and the page fixtures so a new snapshot
// field is defaulted in one place.
import type { CallRow, PluginPage, Stats } from "./snapshot.gen";

export const plugin = (id: string, over: Partial<PluginPage> = {}): PluginPage => ({
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

export const stats = (before: number, after: number, rows: number): Stats => ({
  cache_create: 0,
  cache_read: 0,
  est_before: before,
  est_after: after,
  input: before,
  output: after,
  rows,
});

export const call = (id: number, over: Partial<CallRow> = {}): CallRow => ({
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
  session: "sample-session",
  surface: "hook",
  ts: 1_790_000_000 + id * 30,
  ...over,
});
