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
    }
  | {
      project: ProjectRequest;
    }
  | {
      doctor: DoctorRequest;
    };
/**
 * The registry writes the graph page offers; `<project>` is an id or a
 * root path, as in `rtok graph projects`.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ProjectRequest".
 */
export type ProjectRequest =
  | {
      action: "select";
      project: string;
    }
  | {
      action: "link";
      both: boolean;
      from: string;
      to: string;
    }
  | {
      action: "unlink";
      both: boolean;
      from: string;
      to: string;
    };
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "DoctorAction".
 */
export type DoctorAction = "plan" | "apply";
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
    }
  | {
      plan: Plan;
      type: "doctorplan";
    }
  | {
      fixed: Fixed;
      type: "doctorfixed";
    };
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ModuleState".
 */
export type ModuleState = "installed" | "not_installed" | "not_supported";
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "LinkKind".
 */
export type LinkKind = "manual" | "auto";
/**
 * How a project got into the registry (T329 §1).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Origin".
 */
export type Origin = "manual" | "session" | "worktree" | "mcp" | "reference";

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
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "DoctorRequest".
 */
export interface DoctorRequest {
  action: DoctorAction;
  selection: Selection;
}
/**
 * What the user changed since the defaults.
 */
export interface Selection {
  /**
   * Entries made the kept copy of their duplicate.
   */
  keep: Ref[];
  /**
   * Entries whose selection is the opposite of their default.
   */
  toggled: Ref[];
}
/**
 * An entry as the page names it: the file and the key path inside it.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Ref".
 */
export interface Ref {
  path: string;
  source: string;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Plan".
 */
export interface Plan {
  /**
   * The diff of every file the selection would change.
   */
  diff: string;
  items: Item[];
  /**
   * Selected entries the engine will not remove, with why.
   */
  refused: string[];
}
/**
 * One line of the checklist.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Item".
 */
export interface Item {
  agent: string;
  /**
   * Whether this copy may be made the kept one.
   */
  can_keep: boolean;
  detail: string;
  /**
   * The file that holds the copy kept instead, for a duplicate.
   */
  kept_in: string | null;
  kind: string;
  /**
   * What the entry runs or is called.
   */
  label: string;
  path: string;
  selected: boolean;
  /**
   * In a project's shared file: starts unselected.
   */
  shared: boolean;
  source: string;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Fixed".
 */
export interface Fixed {
  /**
   * 1 when a selected entry was skipped or failed.
   */
  code: number;
  text: string;
}
/**
 * Everything a surface needs for one refresh. `Default` is the empty frame a surface
 * paints while its first read is still running.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Snapshot".
 */
export interface Snapshot {
  agent_usage: UsagePage;
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
   * Project registry (T329.12): every registered project with its index state and links,
   * the same rows `rtok graph projects --json` prints. `None` when the `graph` feature is
   * off or the store read failed.
   */
  projects: ProjectRow[] | null;
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
 * Usage page (T358.5): what `rtok agents usage` reports, through [`usage_page`]. The
 * overview's own `usage` key is the proxy's totals, so this one carries the page's name
 * in its own words.
 */
export interface UsagePage {
  report: Report2 | null;
  text: string;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Report2".
 */
export interface Report2 {
  agents: Agent[];
  /**
   * `--by model` only: the model rows the middle table shows instead of `agents`.
   */
  models?: ModelRow[] | null;
  periods: Period[];
  /**
   * Hosts whose files exist but could not be read: named, counted nowhere.
   */
  skipped: Skipped[];
  source: string;
  /**
   * The last day with usage, in `tz`.
   */
  through: string | null;
  totals: Totals;
  tz: string;
  unpriced: Unpriced[];
  /**
   * Distinct model ids without a price.
   */
  unpriced_models: number;
}
/**
 * Token legs and the estimated cost of one table row. `cost_usd` is `None` when no model in
 * the row has a price: `-` in the table, `null` in JSON, never `$0.00` (that means free).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Agent".
 */
export interface Agent {
  cache_read: number;
  cache_write: number;
  cost_usd: number | null;
  /**
   * `both` only: `through_rtok_tokens` over the logs' tokens, so an agent that bypasses
   * the proxy reads low. `None` when the logs hold no tokens for it.
   */
  coverage?: number | null;
  host: string;
  input: number;
  name: string;
  output: number;
  saved_tokens?: number;
  saved_usd?: number | null;
  /**
   * `both` only: tokens of this agent that also passed through rtok.
   */
  through_rtok_tokens?: number | null;
  tokens: number;
}
/**
 * Token legs and the estimated cost of one table row. `cost_usd` is `None` when no model in
 * the row has a price: `-` in the table, `null` in JSON, never `$0.00` (that means free).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ModelRow".
 */
export interface ModelRow {
  cache_read: number;
  cache_write: number;
  cost_usd: number | null;
  input: number;
  model: string;
  output: number;
  tokens: number;
}
/**
 * Token legs and the estimated cost of one table row. `cost_usd` is `None` when no model in
 * the row has a price: `-` in the table, `null` in JSON, never `$0.00` (that means free).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Period".
 */
export interface Period {
  cache_read: number;
  cache_write: number;
  cost_usd: number | null;
  input: number;
  output: number;
  period: string;
  tokens: number;
}
/**
 * A host whose session files exist but could not be read: named once, counted nowhere.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Skipped".
 */
export interface Skipped {
  host: string;
  path: string;
  reason: string;
}
/**
 * Token legs and the estimated cost of one table row. `cost_usd` is `None` when no model in
 * the row has a price: `-` in the table, `null` in JSON, never `$0.00` (that means free).
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Totals".
 */
export interface Totals {
  cache_read: number;
  cache_write: number;
  cost_usd: number | null;
  /**
   * Distinct `(agent, day)` pairs with usage, what ccusage calls daily rows.
   */
  daily_rows: number;
  input: number;
  output: number;
  saved_tokens?: number;
  saved_usd?: number | null;
  /**
   * Distinct session ids.
   */
  sessions: number;
  tokens: number;
}
/**
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Unpriced".
 */
export interface Unpriced {
  host: string;
  model: string;
  tokens: number;
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
   * Hooks that lead nowhere or cannot be checked (T331.1); the list later detectors extend.
   */
  problems?: Problem[];
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
 * One finding of a doctor config check. The list is shared by every T331 detector.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Problem".
 */
export interface Problem {
  agent: string;
  /**
   * The command as written, never expanded.
   */
  command?: string;
  detail: string;
  event?: string;
  /**
   * Whether `doctor --fix` may remove it (T331.5): only `broken-hook`.
   */
  fixable: boolean;
  /**
   * Copies of one duplicate share a `group`; `keep` marks the copy to keep (T331.3).
   */
  group?: number | null;
  keep?: boolean;
  /**
   * `broken-hook`, `suspect-hook`, `unverified-hook`, `duplicate-hook`, `duplicate-mcp`,
   * `conflicting-mcp`, `stale-plugin` or `unreadable-config`.
   */
  kind: string;
  matcher?: string | null;
  /**
   * The key path of the entry inside `source`.
   */
  path: string;
  /**
   * The config file the entry lives in.
   */
  source: string;
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
 * One registry row as `graph projects` prints it and the `/ws` snapshot carries it.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ProjectRow".
 */
export interface ProjectRow {
  created_at: number;
  id: number;
  index: ProjectIndex | null;
  last_used_at: number;
  links: ProjectLink[];
  missing: boolean;
  name: string;
  origin: Origin;
  root: string;
  selected: boolean;
  state: string;
}
/**
 * `graph status` numbers for one project; absent for a missing root, which has nothing
 * readable to count.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ProjectIndex".
 */
export interface ProjectIndex {
  files: number;
  indexed_at: number | null;
  pending: number;
  rows: number;
  watch: string;
}
/**
 * One outgoing link of a project.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "ProjectLink".
 */
export interface ProjectLink {
  kind: LinkKind;
  name: string;
  reason: string | null;
  to: number;
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
 * What the user changed since the defaults.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "Selection".
 */
export interface Selection1 {
  /**
   * Entries made the kept copy of their duplicate.
   */
  keep: Ref[];
  /**
   * Entries whose selection is the opposite of their default.
   */
  toggled: Ref[];
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
/**
 * The Usage page: one [`usage::Report`] — the call `rtok agents usage` makes — read once and
 * carried twice. `text` is that command's screen ([`usage::Report::to_text`]) for the tui and
 * the Slint page; `report` is the same rows as data for the SPA's tables. Nothing is summed
 * a second time (D27). Both are empty-handed while the first read runs or after it fails:
 * `report` is `None` and `text` says why.
 *
 * This interface was referenced by `WsProtocol`'s JSON-Schema
 * via the `definition` "UsagePage".
 */
export interface UsagePage1 {
  report: Report2 | null;
  text: string;
}
