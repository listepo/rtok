// Text and skills fixtures for the pages whose /ws field is a Rust-rendered text blob. The
// line shapes follow the producers in src/web/model.rs; the values are made up.
import type { SkillPageRow } from "../api/snapshot.gen";

export const statsText = [
  "sessions 42  compact 3  checkpoint 5  no_checkpoint 37  lines 18204  malformed 0",
  "usage input=1204332 cache_create=388120 cache_read=9120554 output=96310  hit=85.0%  median_context=61240",
  "sub-agents 3 sessions / 7 agents  results 90210 B vs parents 410022 B (18.0% of the tree)",
  "cost (USD at [stats.prices] $/MTok; `-` = no price row)",
  "model                           input cache_create   cache_read       output       cost      saved",
  "claude-sonnet-5                902114       301220      7203311        71022      12.84       3.10",
  "claude-haiku-4-5                88410        20110       611223         9120       0.41       0.07",
  "gpt-5                          213808        66790      1306020        16168       1.92       0.38",
  "claude-opus-4-1                   0            0            0            0          -          -",
  "cost total $15.17 (cache reads saved $3.55; 1 model(s) without a price)",
  "thinking blocks 12 bytes 40211 est. tokens 10052 0.0840% of session input",
  "",
  "cache health",
  "session                                   turns   cache_read cache_create  busts",
  "3f9a2c1e-7b41-4d0a-9c55-0e12ab34cd56          88      2210334       101220      1",
  "9c0d4e2a-11f3-4b7e-8a20-5d6e7f809a1b          41       903311        48020      0",
  "a17be5f0-2c9d-4e61-b3a4-7f8091a2b3c4          17       221080        30112      2",
  "  bust turn 41 cause=tools cache_create=48120 cache_read=0",
  "  bust turn 6 cause=tools cache_create=31000 cache_read=0",
  "",
].join("\n");

export const graphText = [
  "root ~/GitHub/listepo/apps/rtok",
  "rows 48213",
  "files 612",
  "pending 2",
  "  src/web/model.rs",
  "  src/tui/view.rs",
  "watch true",
  "indexed_at 2026-09-27 18:12:40",
  "",
  "dead symbols",
  " src/render.rs:212 function pad_right",
  " src/measure/cache.rs:188 function shape_digest",
  " src/plugins/toon/table.rs:77 struct LegacyRow",
  " … 195 more, capped at 200 — `rtok graph dead --json` has the rest",
  "",
].join("\n");

export const hostsText = [
  "CLI: Claude Code",
  "  app     /opt/homebrew/bin/claude (2.1.4)",
  "  config  ~/.claude/settings.json, ~/.claude.json",
  "  hooks   installed",
  "  mcp     installed",
  "  proxy   installed",
  "  plugin  not installed --plugin",
  "CLI: Codex",
  "  app     /opt/homebrew/bin/codex (0.64.0)",
  "  config  ~/.codex/config.toml",
  "  hooks   not supported",
  "  mcp     installed",
  "  proxy   not installed --proxy",
  "Desktop: Cursor",
  "  app     /Applications/Cursor.app (2.3.1)",
  "  config  ~/.cursor/hooks.json, ~/.cursor/mcp.json",
  "  hooks   installed",
  "CLI: Gemini CLI — not found",
  "  app     -",
  "  config  ~/.gemini/settings.json",
  "Desktop: Zed — not installed",
  "  skip    nothing of rtok here; run `rtok agents install zed`",
  "",
].join("\n");

export const configText = [
  "core.enabled = true (default)",
  'core.db_path = "~/.rtok/rtok.db" (default)',
  "core.retain_calls_days = 14 (user)",
  "log.lines = 200 (default)",
  'log.level = "debug" (env)',
  "hook.max_ms = 10 (default)",
  "mcp.tools = [] (default)",
  "proxy.port = 8790 (default)",
  'proxy.mode = "compress" (project)',
  "web.port = 3333 (flag)",
  'demon.services = ["proxy", "web"] (user)',
  "plugins.measure.enabled = false (project)",
  "",
].join("\n");

export const servicesText = [
  "proxy  running  pid=48211  uptime=18342s  log=~/.rtok/demon/proxy.log",
  "mcp  stopped  pid=-  uptime=-  log=~/.rtok/demon/mcp.log",
  "web  running  pid=48230  uptime=18339s  log=~/.rtok/demon/web.log",
  "otel endpoint=http://127.0.0.1:4318 calls_mark=5102 calls_pending=18 logs_mark=2210 logs_pending=0 sessions_mark=311",
  "last flush: [info] otel exported 120 calls, 40 logs",
  "",
].join("\n");

export const worktreesText = [
  "path                                       branch                 owner                   agent               agent state  state     seen  modified   source   cache",
  "~/GitHub/listepo/apps/rtok                 main                   -                       -                   -            main      -     2h         41 MB   6.2 GB",
  "~/GitHub/listepo/_worktrees/rtok-t307      t307-report-sparkline  Claude Code / sonnet-5  1a2b3c4d claude     active       dirty     3m    3m         41 MB   2.8 GB",
  "~/GitHub/listepo/_worktrees/rtok-web       design/web-admin       locked, owner unknown   -                   -            unmerged  -     1d         44 MB      0 B",
  "~/GitHub/listepo/_worktrees/rtok-t305      t305-stats-replay      -                       seen codex 9f8e7d6c  -           merged    4d    4d         41 MB   3.1 GB",
  "~/GitHub/listepo/_worktrees/rtok-t290      -                      -                       -                   -            stale     -     -           0 B      0 B",
  "5 worktrees: 167 MB source, 12.1 GB build cache (logical bytes; clones and hard links count in full)",
  "",
].join("\n");

const skill = (name: string, over: Partial<SkillPageRow> = {}): SkillPageRow => ({
  name,
  source: "user",
  desc_chars: 120,
  body_bytes: 2_600,
  invocations: 4,
  resident: 2_600,
  last_invoked: "2026-09-27 16:03",
  never: false,
  ...over,
});

export const skillRows: SkillPageRow[] = [
  skill("worktrees", { desc_chars: 212, body_bytes: 3_400, invocations: 9 }),
  skill("release", {
    source: "project",
    body_bytes: 11_200,
    invocations: 0,
    resident: 0,
    last_invoked: "-",
    never: true,
  }),
  skill("rtok-expand", { source: "project", desc_chars: 96, invocations: 31 }),
  skill("review-pr", { desc_chars: 188, body_bytes: 5_200 }),
];

export const skillsHeader =
  "4 listed · desc 596 B ≈ 149 tok/req · resident 11.4 KB · 0.3% of input";
