// Parsers for the Snapshot fields that are plain text on the wire: stats, graph, hosts,
// config, services and worktrees. Rust renders each page as the same text its CLI command
// prints (D27), so the SPA reads that text instead of a second structured contract. Every
// parser keeps what it does not understand (`other`) so a new line in the Rust output shows
// up on the page instead of vanishing.

/** `key=value` pairs, as in `usage input=1 hit=85.0%`. */
export function kvPairs(line: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of line.matchAll(/([a-z_]+)=(\S+)/g)) out[m[1]!] = m[2]!;
  return out;
}

/** `key 123` pairs, as in `sessions 42  compact 3`. */
export function kvSpaced(line: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const m of line.matchAll(/([a-z_]+) (\d+)/g)) out[m[1]!] = m[2]!;
  return out;
}

/** Table cells: the Rust tables pad columns with two or more spaces. */
export const cells = (line: string): string[] => line.trim().split(/\s{2,}/);

const num = (v: string | undefined): number | null => {
  if (v == null || v === "-" || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
};

export function humanSecs(s: number | null): string {
  if (s == null || !Number.isFinite(s)) return "-";
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h}h ${m}m` : `${m}m ${s % 60}s`;
}

const lines = (t: string): string[] => t.split("\n");

// -- stats

export interface PricedRow {
  model: string;
  input: number | null;
  cacheCreate: number | null;
  cacheRead: number | null;
  output: number | null;
  /** `null` is the `-` the report prints for a model without a price row. */
  cost: number | null;
  saved: number | null;
}

export interface HealthRow {
  session: string;
  turns: number | null;
  cacheRead: number | null;
  cacheCreate: number | null;
  busts: number | null;
}

export interface Bust {
  turn: string;
  cause: string;
  cacheCreate: number | null;
  cacheRead: number | null;
}

export interface StatsView {
  totals: Record<string, string>;
  usage: Record<string, string>;
  priced: PricedRow[];
  /** From the `cost total` line when the report has one. */
  costTotal: { cost: number; saved: number } | null;
  health: HealthRow[];
  busts: Bust[];
  /** Lines the page does not chart (sub-agents, edits, thinking, ...), verbatim. */
  other: string[];
}

export function parseStats(text: string): StatsView {
  const L = lines(text);
  const used = new Set<number>();
  const take = (i: number) => {
    if (i >= 0) used.add(i);
    return i;
  };
  const find = (prefix: string) => take(L.findIndex((l) => l.startsWith(prefix)));
  const at = (i: number) => (i >= 0 ? (L[i] ?? "") : "");

  const totals = kvSpaced(at(find("sessions ")));
  const usage = kvPairs(at(find("usage ")));

  const priced: PricedRow[] = [];
  let costTotal: StatsView["costTotal"] = null;
  const iModel = find("model ");
  take(L.findIndex((l) => l.startsWith("cost (USD")));
  if (iModel >= 0) {
    for (let i = iModel + 1; i < L.length; i++) {
      const l = L[i] ?? "";
      const total = /^cost total \$([\d.]+) \(cache reads saved \$([\d.]+)/.exec(l);
      if (total) {
        costTotal = { cost: Number(total[1]), saved: Number(total[2]) };
        used.add(i);
        break;
      }
      const c = cells(l);
      if (!l.trim() || c.length < 7) break;
      used.add(i);
      priced.push({
        model: c[0]!,
        input: num(c[1]),
        cacheCreate: num(c[2]),
        cacheRead: num(c[3]),
        output: num(c[4]),
        cost: num(c[5]),
        saved: num(c[6]),
      });
    }
  }

  const health: HealthRow[] = [];
  const busts: Bust[] = [];
  const iCache = take(L.indexOf("cache health"));
  if (iCache >= 0) {
    take(iCache + 1); // column header
    for (let i = iCache + 2; i < L.length; i++) {
      const l = L[i] ?? "";
      if (!l.trim()) continue;
      if (l.startsWith("  bust")) {
        const k = kvPairs(l);
        busts.push({
          turn: /turn (\d+)/.exec(l)?.[1] ?? "?",
          cause: k.cause ?? "?",
          cacheCreate: num(k.cache_create),
          cacheRead: num(k.cache_read),
        });
        used.add(i);
        continue;
      }
      const c = cells(l);
      if (c.length < 5) continue; // e.g. `no usage rows`: stays in `other`
      used.add(i);
      health.push({
        session: c[0]!,
        turns: num(c[1]),
        cacheRead: num(c[2]),
        cacheCreate: num(c[3]),
        busts: num(c[4]),
      });
    }
  }

  const other = L.filter((l, i) => !used.has(i) && l.trim() !== "");
  return { totals, usage, priced, costTotal, health, busts, other };
}

// -- graph

export interface DeadSymbol {
  path: string;
  line: string;
  kind: string;
  name: string;
}

export interface GraphView {
  root: string;
  rows: number | null;
  files: number | null;
  pending: string[];
  watch: boolean;
  indexedAt: string;
  dead: DeadSymbol[];
  /** The ` … N more, capped at ...` or ` dead scan failed` line, verbatim. */
  deadNote: string | null;
  other: string[];
}

export function parseGraph(text: string): GraphView {
  const L = lines(text);
  const used = new Set<number>();
  const get = (key: string): string => {
    const i = L.findIndex((l) => l.startsWith(`${key} `));
    if (i < 0) return "-";
    used.add(i);
    return (L[i] ?? "").slice(key.length + 1);
  };
  const root = get("root");
  const rows = num(get("rows"));
  const files = num(get("files"));
  const watch = get("watch") === "true";
  const indexedAt = get("indexed_at");

  const pending: string[] = [];
  const iPending = L.findIndex((l) => l.startsWith("pending "));
  if (iPending >= 0) {
    used.add(iPending);
    for (let i = iPending + 1; (L[i] ?? "").startsWith("  "); i++) {
      used.add(i);
      pending.push((L[i] ?? "").trim());
    }
  }

  const dead: DeadSymbol[] = [];
  let deadNote: string | null = null;
  const iDead = L.indexOf("dead symbols");
  if (iDead >= 0) {
    used.add(iDead);
    for (let i = iDead + 1; i < L.length; i++) {
      const l = L[i] ?? "";
      const m = /^ (\S+):(\d+) (\S+) (.+)$/.exec(l);
      if (m) {
        used.add(i);
        dead.push({ path: m[1]!, line: m[2]!, kind: m[3]!, name: m[4]! });
      } else if (l.trim() === "none") {
        used.add(i);
      } else if (/^ (…|dead scan failed)/.test(l)) {
        used.add(i);
        deadNote = l.trim();
      }
    }
  }
  const other = L.filter((l, i) => !used.has(i) && l.trim() !== "");
  return { root, rows, files, pending, watch, indexedAt, dead, deadNote, other };
}

// -- hosts

export interface HostBlock {
  kind: string;
  name: string;
  note: string;
  app: string | null;
  config: string[];
  skip: string | null;
  /** Module rows (`hooks`, `mcp`, `proxy`, `plugin`, ...) as `[name, state]`. */
  modules: [string, string][];
}

export interface HostsView {
  /** The first probe runs in the background; until it lands the text says so. */
  probing: boolean;
  blocks: HostBlock[];
  other: string[];
}

export function parseHosts(text: string): HostsView {
  if (text.startsWith("probing hosts")) return { probing: true, blocks: [], other: [] };
  const blocks: HostBlock[] = [];
  const other: string[] = [];
  for (const l of lines(text)) {
    if (!l.trim()) continue;
    const h = /^(CLI|Desktop): (.+?)(?: — (.+))?$/.exec(l);
    if (h) {
      blocks.push({
        kind: h[1]!,
        name: h[2]!,
        note: h[3] ?? "",
        app: null,
        config: [],
        skip: null,
        modules: [],
      });
      continue;
    }
    const m = /^ {2}(\S+)\s+(.*)$/.exec(l);
    const block = blocks[blocks.length - 1];
    if (!m || !block) {
      other.push(l);
      continue;
    }
    const [, key, value] = m as unknown as [string, string, string];
    if (key === "app") block.app = value === "-" ? null : value;
    else if (key === "config") block.config = value.split(", ");
    else if (key === "skip") block.skip = value;
    else block.modules.push([key, value]);
  }
  return { probing: false, blocks, other };
}

// -- config

export const CONFIG_SOURCES = ["default", "user", "project", "env", "flag"] as const;

export interface ConfigEntry {
  key: string;
  value: string;
  source: string;
}

export function parseConfig(text: string): ConfigEntry[] {
  const out: ConfigEntry[] = [];
  for (const l of lines(text)) {
    const m = /^(\S+) = (.*) \((\w+)\)$/.exec(l);
    if (m) out.push({ key: m[1]!, value: m[2]!, source: m[3]! });
  }
  return out;
}

export function matchesConfig(e: ConfigEntry, source: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  return (
    (source === "all" || e.source === source) &&
    (!q || `${e.key} ${e.value}`.toLowerCase().includes(q))
  );
}

/** Rows grouped by their table (`proxy.port` is in `proxy`), in first-seen order. */
export function groupConfig(rows: ConfigEntry[]): [string, ConfigEntry[]][] {
  const groups = new Map<string, ConfigEntry[]>();
  for (const r of rows) {
    const g = r.key.split(".")[0]!;
    groups.set(g, [...(groups.get(g) ?? []), r]);
  }
  return [...groups];
}

// -- services

export interface ServiceRow {
  name: string;
  running: boolean;
  pid: string | null;
  uptimeSecs: number | null;
  log: string | null;
}

export interface OtelView {
  endpoint: string | null;
  callsMark: number | null;
  callsPending: number | null;
  logsMark: number | null;
  logsPending: number | null;
  sessionsMark: number | null;
}

export interface ServicesView {
  services: ServiceRow[];
  otel: OtelView | null;
  lastFlush: string | null;
  other: string[];
}

export function parseServices(text: string): ServicesView {
  const services: ServiceRow[] = [];
  const other: string[] = [];
  let otel: OtelView | null = null;
  let lastFlush: string | null = null;
  for (const l of lines(text)) {
    if (!l.trim()) continue;
    const svc = /^(\S+) {2}(running|stopped) {2}(.*)$/.exec(l);
    if (svc) {
      const k = kvPairs(svc[3]!);
      services.push({
        name: svc[1]!,
        running: svc[2] === "running",
        pid: k.pid && k.pid !== "-" ? k.pid : null,
        uptimeSecs: num(k.uptime?.replace(/s$/, "")),
        log: /log=(.*)$/.exec(l)?.[1] ?? null,
      });
    } else if (l.startsWith("otel ")) {
      const k = kvPairs(l);
      otel = {
        endpoint: k.endpoint && k.endpoint !== "-" ? k.endpoint : null,
        callsMark: num(k.calls_mark),
        callsPending: num(k.calls_pending),
        logsMark: num(k.logs_mark),
        logsPending: num(k.logs_pending),
        sessionsMark: num(k.sessions_mark),
      };
    } else if (l.startsWith("last flush: ")) {
      lastFlush = l;
    } else {
      other.push(l);
    }
  }
  return { services, otel, lastFlush, other };
}

// -- worktrees

export const WORKTREE_STATES = ["main", "dirty", "unmerged", "merged", "stale", "orphan"] as const;

export interface WorktreeRow {
  path: string;
  branch: string;
  owner: string;
  agent: string;
  agentState: string;
  state: string;
  seen: string;
  modified: string;
  source: string;
  cache: string;
}

export type WorktreesView =
  /** `reading worktrees…` or `not a git repository`: an ordinary state, not a failure. */
  | { kind: "message"; message: string }
  | { kind: "table"; rows: WorktreeRow[]; total: string; note: string };

// Header names of `worktree::list::to_table` mapped to row fields; a column the header
// does not have (an older server) reads as `-`.
const WORKTREE_COLUMNS: Record<string, keyof WorktreeRow> = {
  path: "path",
  branch: "branch",
  owner: "owner",
  agent: "agent",
  "agent state": "agentState",
  state: "state",
  seen: "seen",
  modified: "modified",
  source: "source",
  cache: "cache",
};

export function parseWorktrees(text: string): WorktreesView {
  const L = lines(text).filter((l) => l.trim());
  const first = L[0] ?? "";
  if (!first || first.startsWith("reading worktrees") || first.startsWith("not a git repository"))
    return { kind: "message", message: first.trim() || "no worktrees" };
  const header = cells(first);
  const totalLine = L.find((l) => / worktrees: /.test(l)) ?? "";
  const rows = L.slice(1)
    .filter((l) => l !== totalLine)
    .map((l) => {
      const c = cells(l);
      const row: WorktreeRow = {
        path: "-",
        branch: "-",
        owner: "-",
        agent: "-",
        agentState: "-",
        state: "-",
        seen: "-",
        modified: "-",
        source: "-",
        cache: "-",
      };
      header.forEach((h, i) => {
        const field = WORKTREE_COLUMNS[h];
        if (field) row[field] = c[i] ?? "-";
      });
      return row;
    });
  return {
    kind: "table",
    rows,
    total: totalLine.replace(/ \(.*$/, ""),
    note: /\((.*)\)/.exec(totalLine)?.[1] ?? "",
  };
}

export function matchesWorktree(r: WorktreeRow, state: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  return (
    (state === "all" || r.state === state) &&
    (!q || `${r.path} ${r.branch} ${r.owner}`.toLowerCase().includes(q))
  );
}
