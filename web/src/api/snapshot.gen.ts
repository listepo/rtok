// Generated from ws.schema.json by scripts/gen-api.mjs (T310.2). Do not edit: change the Rust
// types in src/web, bless the schema, then run `npm run gen:api`.

/**
 * A message a client sends over `/ws`.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ClientMessage".
 */
export type ClientMessage =
  | {
      expand: string;
    }
  | {
      set: SetRequest;
    };
/**
 * A frame the server pushes besides the [`Snapshot`] itself.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ServerFrame".
 */
export type ServerFrame =
  | {
      text: string;
      type: "message";
    }
  | {
      id: string;
      text: string;
      type: "expand";
    };
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ModuleState".
 */
export type ModuleState = "installed" | "not_installed" | "not_supported";

/**
 * Root of the schema: one property per direction, so every type lands in `$defs` once.
 */
export interface WsProtocol {
  client: ClientMessage;
  server: ServerFrame;
  snapshot: Snapshot;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SetRequest".
 */
export interface SetRequest {
  key: string;
  value: boolean;
}
/**
 * Everything a surface needs for one refresh. `Default` is the empty frame a surface
 * paints while its first read is still running.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Snapshot".
 */
export interface Snapshot {
  /**
   * Calls page (T15.5): the last [`CALLS_ROWS`] ledger rows, newest first.
   */
  calls: CallRow[];
  /**
   * Config page (T228): `rtok config show --sources`'s rows, through
   * [`config_page_text`] (D27, no second layering). `None` on a failed tick.
   */
  config: string | null;
  /**
   * Doctor page (T15.6): what `rtok doctor` reports — hooks, MCP servers, proxy
   * chains. `None` when this tick's probes failed; the page renders the failure
   * rather than zeros, the way an unreadable store renders an empty page.
   */
  doctor: Report | null;
  /**
   * Set when the store will not open or this tick's doctor probe failed (T60.6). Both
   * surfaces render it as a banner instead of an empty page.
   */
  error?: string | null;
  /**
   * Graph page (T230): `graph status`'s index health (rows, files, the T68.3
   * pending/staleness set, `indexed_at`) plus `graph dead`'s unreferenced-definition
   * list, folded into one page (D27) — [`graph_page_text`], read from the store on
   * this tick (no re-indexing). `None` when the `graph` feature is off or the store
   * read failed.
   */
  graph: string | null;
  /**
   * Hosts page (T231): `agents list`'s blocks — kind, detected version, installed
   * surfaces, config path — one per known host variant (D27), so `agents list` /
   * `agents info` can join `COMMAND_PAGES`. [`hosts_page_text`] reuses the same
   * probe `agents_list`'s JSON form calls (T168's `--version` noise filter and all)
   * behind a cache: never blocks a 2 s tick on a cold or stale probe — the tick
   * renders the last known text, or "probing hosts…" before the first one lands.
   */
  hosts: string;
  /**
   * Logs page (T15.7): the last `[log] lines` log lines, newest first — the same
   * selection `rtok logs` screens ([`Model::log_lines`], T15.11). Riding the snapshot
   * makes the page both surfaces' (D23); `[log] lines` is the frame's bound too.
   */
  logs: string[];
  /**
   * Plugins page: one entry per catalogue plugin.
   */
  plugins: PluginPage[];
  /**
   * Archive ids keyed by `calls[].id` (T60.4). Both surfaces read this map; neither
   * queries the store for an expand handle (D23 / D27).
   */
  ref_ids: {
    /**
     * This interface was referenced by `undefined`'s JSON-Schema definition
     * via the `patternProperty` "^-?\d+$".
     */
    [k: string]: string;
  };
  /**
   * Services page (T229): `demon status`'s per-service rows plus `otel status`'s
   * exporter health, through [`services_page_text`] (D27, no second reader —
   * [`Model::demon`] and [`otel_status`] already build both). `None` on a failed
   * tick.
   */
  services: string | null;
  /**
   * Sessions page (T25.1): one row per session, newest first.
   */
  sessions: SessionTotals[];
  skills: SkillsPage;
  /**
   * Stats page (T227): `stats --price`'s table plus `stats --cache`'s table, folded
   * into one page (D27) — [`stats_page_text`], built from the same transcript scan
   * [`stats_skills`] already runs for the Skills page (no second aggregation). `None`
   * when this tick's transcripts read failed.
   */
  stats: string | null;
  type: string;
  usage: Overview;
  /**
   * Worktrees page (T232): `worktree list`'s rows — path, branch, owner, age,
   * `target/` size and state — through [`worktrees_page_text`], the same
   * [`crate::worktree::list::rows`]/[`crate::worktree::list::to_table`] `worktree
   * list` already calls (D27, no second reader or directory walk); `gc`/`clean`
   * stay CLI-only verdicts. `None` only when the current directory is unreadable.
   */
  worktrees: string | null;
}
/**
 * One `calls` row as the Calls page serves it ([`Store::recent_calls`], T15.5): the
 * ledger's own columns plus the slugs its ids point at and — when the call is one
 * that recorded usage — the newest `usage` row linked to it. One query's output, so
 * no renderer can re-derive a field differently (D27); `api` `None` means no usage
 * row is linked (a hook, MCP call or plugin run carries none).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "CallRow".
 */
export interface CallRow {
  api: string | null;
  cache_create: number | null;
  cache_read: number | null;
  error: string | null;
  host: string | null;
  id: number;
  input: number | null;
  kind: string;
  model: string | null;
  ms: number | null;
  name: string | null;
  ok: number;
  output: number | null;
  parent_id: number | null;
  plugin: string | null;
  provider: string | null;
  session: string;
  surface: string;
  ts: number;
}
/**
 * What `rtok doctor` found, as data.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Report".
 */
export interface Report {
  /**
   * Every host variant and the state of each rtok module in it, as `agent setup` prints.
   */
  agents: AgentModules[];
  auto_compact_window: string | null;
  bash_max_output_length: string | null;
  hooks_by_event: {
    [k: string]: number;
  };
  hooks_total: number;
  /**
   * The instruction audit (T7.2): `Some` only when `[doctor] instructions` ran — an audit
   * that found nothing still prints its section header, as it always did.
   */
  instructions: Instructions | null;
  /**
   * One probed MCP server: name, command, tool count, description tokens.
   */
  mcp: ServerInfo[];
  /**
   * `ANTHROPIC_BASE_URL` is set, so MCP tool search is likely disabled.
   */
  mcp_tool_search_disabled: boolean;
  /**
   * Host-native features that duplicate a running rtok surface (T59.7), each
   * naming the rtok config key that turns the duplicate side off. Advice: the
   * lines say "duplicate", never "saves N".
   */
  overlaps?: string[];
  /**
   * The proxy chain behind `ANTHROPIC_BASE_URL`, hops joined with `→`.
   */
  proxy: string;
  /**
   * The proxy chain behind the OpenAI seed (`OPENAI_BASE_URL` or a host config).
   */
  proxy_openai: string;
  /**
   * Read-class token share from the transcripts (`None` = no data, fail open).
   */
  read_share?: ReadShare | null;
  /**
   * The skills audit (T61.3): `Some` when the probe ran; an empty tree prints no section.
   */
  skills?: SkillsAudit | null;
  /**
   * Advice for enabling `[proxy.tools_rewrite]` when applicable (T59.5).
   */
  tools_rewrite_advice?: string | null;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "AgentModules".
 */
export interface AgentModules {
  host: string;
  kind: string;
  modules: ModuleRow[];
}
/**
 * One [`MODULES`] row of a host variant: its state and the note printed after it — the flag
 * that would install it, or the reason it cannot be installed.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ModuleRow".
 */
export interface ModuleRow {
  name: string;
  note: string;
  state: ModuleState;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Instructions".
 */
export interface Instructions {
  duplicates: [unknown, unknown][];
  rows: InstructionRow[];
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "InstructionRow".
 */
export interface InstructionRow {
  name: string;
  path: string;
  tokens: number;
  warn: boolean;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ServerInfo".
 */
export interface ServerInfo {
  cmd: string;
  desc_tokens: number;
  name: string;
  tools: number;
}
/**
 * Share of Read-class transcript tokens spent in native Grep/Glob (T50.4):
 * `(grep + glob) / (read + grep + glob)` by estimated tokens. `None` when the
 * transcripts hold no Read-class results — the default stays off on no data.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ReadShare".
 */
export interface ReadShare {
  glob_tokens: number;
  grep_tokens: number;
  read_tokens: number;
  share: number;
}
/**
 * The skills audit (T61.3): what the host lists and what it costs the system
 * prompt. Advice only — nothing here edits a file.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SkillsAudit".
 */
export interface SkillsAudit {
  /**
   * Description bytes the listing rides with every request (≈ tokens/4).
   */
  desc_bytes: number;
  rows: SkillRow[];
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SkillRow".
 */
export interface SkillRow {
  body_bytes: number;
  desc_chars: number;
  /**
   * Invocations in the last 30 d from the T61.1 fold; `None` = no data.
   */
  invocations?: number | null;
  name: string;
  /**
   * `user`, `project`, or `plugin:<id>@<marketplace>`.
   */
  source: string;
  /**
   * Body over 8 KB — almost always a `references/` candidate.
   */
  warn_body: boolean;
  /**
   * `description:` over the measured 200-char median.
   */
  warn_desc: boolean;
  /**
   * Listed but never invoked in the window (only when data exists).
   */
  warn_never: boolean;
}
/**
 * A plugin's page: its manifest, the static copy it contributes through
 * `Plugin::dashboard_page`, and the stats widget when it saves tokens.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "PluginPage".
 */
export interface PluginPage {
  enabled: boolean;
  fields: [unknown, unknown][];
  id: string;
  saves_tokens: boolean;
  stats: Stats | null;
  summary: string;
  surfaces: string[];
  title: string;
}
/**
 * The shared stats widget: `usage` rows for the overview, `Measurement` rows per plugin.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Stats".
 */
export interface Stats {
  cache_create: number;
  cache_read: number;
  est_after: number;
  est_before: number;
  input: number;
  output: number;
  rows: number;
}
/**
 * One session's totals ([`Store::session_totals`], T25.1) — the rendering input of the
 * Sessions page and `rtok agent sessions` (T25.2). Everything below is one query's
 * output, so no renderer can re-derive a number differently (D27): tokens are whole-
 * session sums of `usage`, `last_activity` is the MAX ts over the session's `usage`
 * and `calls` rows (falling back to `started_at` when there are none), and `ended_at`
 * `None` means live.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SessionTotals".
 */
export interface SessionTotals {
  api: string | null;
  cache_create: number;
  cache_read: number;
  ended_at: number | null;
  host: string | null;
  id: string;
  input: number;
  last_activity: number;
  model: string | null;
  output: number;
  project: string | null;
  provider: string | null;
  started_at: number;
}
/**
 * Skills page (T63.1): T61.3 listing joined to T61.1 resident/invocations.
 */
export interface SkillsPage {
  /**
   * Totals: listed, desc bytes ≈ tokens/req (chars/4, research.md §10.2), resident, input share.
   */
  header: string;
  rows: SkillPageRow[];
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SkillPageRow".
 */
export interface SkillPageRow {
  body_bytes: number;
  desc_chars: number;
  invocations: number;
  last_invoked: string;
  name: string;
  never: boolean;
  resident: number;
  source: string;
}
/**
 * Overview page: provider usage totals, CTT and the per-turn series.
 */
export interface Overview {
  /**
   * Persistent operator warnings for the whole disabled period (e.g. proxy plain
   * mode). Empty when none; omitted from the wire when empty so the P19 shape stays.
   */
  alerts?: string[];
  cache_create: number;
  cache_read: number;
  /**
   * Σ over sessions of Σ over turns `ctx × turns-after`, where `ctx` is the turn's
   * input-side tokens (`input + cache_create + cache_read`) and `turns-after` counts
   * the session's later turns — the usage-row mirror of `stats`' `tokens × remain`.
   * Output is left out on purpose: it re-enters as a later turn's input or cache,
   * so counting it again would count it twice.
   */
  ctt: number;
  est_after: number;
  est_before: number;
  input: number;
  output: number;
  rows: number;
  /**
   * Per-turn `ctx`, sessions oldest-first and request order within a session, kept
   * to the last [`OVERVIEW_TURNS`]. A `usage` row carries no timestamp, so across
   * sessions this is session order, not wall-clock order.
   */
  turns: number[];
}
/**
 * The Overview page (T15.3): the usage totals plus what the tab draws from them —
 * context-token-turns and the per-turn series behind the sparkline. The totals stay
 * flat under the `usage` key, so the `/ws` frame keeps the shape P19 pinned and the
 * SPA reads on untouched.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Overview".
 */
export interface Overview1 {
  /**
   * Persistent operator warnings for the whole disabled period (e.g. proxy plain
   * mode). Empty when none; omitted from the wire when empty so the P19 shape stays.
   */
  alerts?: string[];
  cache_create: number;
  cache_read: number;
  /**
   * Σ over sessions of Σ over turns `ctx × turns-after`, where `ctx` is the turn's
   * input-side tokens (`input + cache_create + cache_read`) and `turns-after` counts
   * the session's later turns — the usage-row mirror of `stats`' `tokens × remain`.
   * Output is left out on purpose: it re-enters as a later turn's input or cache,
   * so counting it again would count it twice.
   */
  ctt: number;
  est_after: number;
  est_before: number;
  input: number;
  output: number;
  rows: number;
  /**
   * Per-turn `ctx`, sessions oldest-first and request order within a session, kept
   * to the last [`OVERVIEW_TURNS`]. A `usage` row carries no timestamp, so across
   * sessions this is session order, not wall-clock order.
   */
  turns: number[];
}
/**
 * Skills page (T63.1, D23): one row per skill the host lists.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "SkillsPage".
 */
export interface SkillsPage1 {
  /**
   * Totals: listed, desc bytes ≈ tokens/req (chars/4, research.md §10.2), resident, input share.
   */
  header: string;
  rows: SkillPageRow[];
}
