// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Usage page data (T358.5) for `?sample`, stories and tests. Every number is made up; the shapes
// are `rtok agents usage --json`'s, and the rows add up the way the Rust report's do.
import type { UsagePage } from "../api/snapshot.gen";
import type { UsageReport } from "./model";

// `--source both`: logs from three agents and what passed through rtok, in two currencies of
// completeness: Gemini CLI's model has no price, so its cost is unknown, never zero.
export const usageBoth: UsageReport = {
  source: "both",
  tz: "UTC",
  through: "2026-10-03",
  totals: {
    tokens: 65_500_000,
    input: 3_500_000,
    cache_write: 3_400_000,
    cache_read: 57_000_000,
    output: 1_600_000,
    cost_usd: 450.63,
    sessions: 214,
    daily_rows: 61,
    saved_tokens: 4_100_000,
    saved_usd: 18.4,
  },
  unpriced_models: 1,
  unpriced: [{ model: "gemini-3-pro-preview", host: "gemini", tokens: 400_000 }],
  agents: [
    {
      host: "claude",
      name: "Claude Code",
      tokens: 53_500_000,
      input: 1_200_000,
      cache_write: 3_400_000,
      cache_read: 48_000_000,
      output: 900_000,
      cost_usd: 412.73,
      through_rtok_tokens: 31_000_000,
      coverage: 0.58,
      saved_tokens: 4_100_000,
      saved_usd: 18.4,
    },
    {
      host: "codex",
      name: "Codex",
      tokens: 11_600_000,
      input: 2_000_000,
      cache_write: 0,
      cache_read: 9_000_000,
      output: 600_000,
      cost_usd: 37.9,
      through_rtok_tokens: 0,
      coverage: 0,
      saved_tokens: 0,
      saved_usd: null,
    },
    {
      host: "gemini",
      name: "Gemini CLI",
      tokens: 400_000,
      input: 300_000,
      cache_write: 0,
      cache_read: 0,
      output: 100_000,
      cost_usd: null,
      through_rtok_tokens: 0,
      coverage: 0,
      saved_tokens: 0,
      saved_usd: null,
    },
  ],
  periods: [
    {
      period: "2026-08",
      tokens: 21_000_000,
      input: 1_000_000,
      cache_write: 1_100_000,
      cache_read: 18_400_000,
      output: 500_000,
      cost_usd: 160.2,
    },
    {
      period: "2026-09",
      tokens: 31_500_000,
      input: 1_700_000,
      cache_write: 1_500_000,
      cache_read: 27_800_000,
      output: 500_000,
      cost_usd: 201.4,
    },
    {
      period: "2026-10",
      tokens: 13_000_000,
      input: 800_000,
      cache_write: 800_000,
      cache_read: 10_800_000,
      output: 600_000,
      cost_usd: 89.03,
    },
  ],
  skipped: [],
};

// `--source logs`: no rtok columns, no savings; one host's files could not be read.
export const usageLogs: UsageReport = {
  ...usageBoth,
  source: "logs",
  totals: {
    ...usageBoth.totals,
    saved_tokens: undefined,
    saved_usd: undefined,
  },
  agents: usageBoth.agents.map((a) => ({
    host: a.host,
    name: a.name,
    tokens: a.tokens,
    input: a.input,
    cache_write: a.cache_write,
    cache_read: a.cache_read,
    output: a.output,
    cost_usd: a.cost_usd,
  })),
  skipped: [{ host: "opencode", reason: "unreadable", path: "~/.local/share/opencode/storage" }],
};

// `--by model` replaces the agent table with one row per model.
export const usageByModel: UsageReport = {
  ...usageLogs,
  skipped: [],
  models: [
    {
      model: "claude-sonnet-5-5",
      tokens: 49_000_000,
      input: 1_000_000,
      cache_write: 3_000_000,
      cache_read: 44_400_000,
      output: 600_000,
      cost_usd: 351.2,
    },
    {
      model: "gpt-5.5",
      tokens: 11_600_000,
      input: 2_000_000,
      cache_write: 0,
      cache_read: 9_000_000,
      output: 600_000,
      cost_usd: 37.9,
    },
    {
      model: "claude-haiku-4-5",
      tokens: 4_500_000,
      input: 200_000,
      cache_write: 400_000,
      cache_read: 3_600_000,
      output: 300_000,
      cost_usd: 61.53,
    },
    {
      model: "gemini-3-pro-preview",
      tokens: 400_000,
      input: 300_000,
      cache_write: 0,
      cache_read: 0,
      output: 100_000,
      cost_usd: null,
    },
  ],
};

// Nothing recorded yet: the server still answers, with no `through` day.
export const usageEmpty: UsageReport = {
  ...usageBoth,
  source: "rtok",
  through: null,
  totals: {
    ...usageBoth.totals,
    tokens: 0,
    input: 0,
    cache_write: 0,
    cache_read: 0,
    output: 0,
    cost_usd: null,
    sessions: 0,
    daily_rows: 0,
    saved_tokens: undefined,
    saved_usd: undefined,
  },
  unpriced_models: 0,
  unpriced: [],
  agents: [],
  periods: [],
};

export const usageText = [
  "rtok agents usage: logs from 3 agents and through rtok, up to 2026-10-03 (UTC)",
  "",
  "  65.5M tokens",
  "  $450.63 estimated cost",
  "  214 sessions",
  "  61 daily rows",
  "  rtok saved 4.1M tokens (≈ $18.40)",
  "",
  "! Cost is incomplete: 1 model has no price in [stats.prices], so its tokens are not in",
  "  the estimate. `rtok agents usage --unpriced` lists them.",
  "",
].join("\n");

export const usagePage = (report: UsageReport, text = usageText): UsagePage => ({ text, report });

// The first read of agent logs is still running (src/web/model.rs `usage_page`).
export const usageReading: UsagePage = { text: "reading usage…\n", report: null };

export const usageFailed: UsagePage = {
  text: "usage did not answer this tick: unknown time zone Nowhere/Land\n",
  report: null,
};
