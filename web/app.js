// rtok web admin — plain JS, no deps. Hash-routed single page.
// Data contract: the `/ws` snapshot JSON (src/web/model.rs `Snapshot`), the same
// frame crates/rtok-webui parses. When no rtok server answers, SAMPLE data shaped
// exactly like that frame is shown and labelled "sample data" everywhere.
(function () {
  "use strict";

  // F Signal/scope icon set — verbatim copies of crates/rtok-webui/ui/icons/*.svg.
  const ICONS = {
    calls:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M4 14c2-4 4-6 8-6s6 2 8 6"/><path d="M7 14c1.2-2.2 2.5-3.2 5-3.2S15.8 11.8 17 14"/><circle cx="12" cy="16.5" r="1.5" fill="currentColor" stroke="none"/><path d="M19 7l1.5-1.5" stroke="currentColor"/></svg>',
    doctor:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M3 12h3l2-6 3 12 2-8 2 4h6"/><circle cx="20" cy="12" r="1.5" fill="currentColor" stroke="none"/></svg>',
    logs: '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M5 5v14"/><path d="M5 7h6"/><path d="M5 11h10"/><path d="M5 15h8"/><path d="M5 19h12"/><path d="M15 7h4" stroke="currentColor"/></svg>',
    overview:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="4"/><circle cx="12" cy="12" r="1.2" fill="currentColor" stroke="none"/><path d="M12 4v2M20 12h-2M12 20v-2M4 12h2"/></svg>',
    plugins:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><circle cx="8" cy="8" r="2.5"/><circle cx="16" cy="8" r="2.5"/><circle cx="12" cy="16" r="2.5"/><path d="M10 9.2 11.2 14"/><path d="M14 9.2 12.8 14"/><path d="M10.5 8h3"/></svg>',
    savings:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M4 16c3-6 5-9 8-9s5 3 8 9"/><path d="M8 16c1.5-3 2.5-4.5 4-4.5s2.5 1.5 4 4.5"/><path d="M12 16v3" stroke="currentColor"/></svg>',
    sessions:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><rect x="3" y="6" width="18" height="12" rx="2"/><path d="M3 10h18"/><circle cx="7" cy="8" r="0.9" fill="currentColor" stroke="none"/><circle cx="10" cy="8" r="0.9" fill="currentColor" stroke="none"/><path d="M7 13h4M7 15.5h7"/></svg>',
    skills:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M2.5 9.7 12 5.5 21.5 9.7 12 13.9z"/><path d="M6.5 11.8v4.3c0 1.5 2.5 2.6 5.5 2.6s5.5-1.1 5.5-2.6v-4.3"/><path d="M21.5 9.7v4.4"/><circle cx="21.5" cy="15.5" r="1.2" fill="currentColor" stroke="none"/></svg>',
    theme:
      '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round"><path d="M12 3v3"/><path d="M12 18v3"/><path d="M5.6 5.6l2.1 2.1"/><path d="M16.3 16.3l2.1 2.1"/><circle cx="12" cy="12" r="5"/><path d="M12 9.5a2.5 2.5 0 1 0 2.5 2.5" fill="currentColor" fill-opacity="0.25" stroke="none"/></svg>',
  };

  // ---------------------------------------------------------------- utils
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const esc = (s) =>
    String(s == null ? "" : s).replace(
      /[&<>"']/g,
      (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c],
    );
  const nf = new Intl.NumberFormat("en-US");
  const fmt = (n) => (n == null ? "—" : nf.format(n));
  const compact = (n) => {
    if (n == null) return "—";
    const a = Math.abs(n);
    if (a >= 1e9) return (n / 1e9).toFixed(a >= 1e10 ? 0 : 1) + "B";
    if (a >= 1e6) return (n / 1e6).toFixed(a >= 1e7 ? 0 : 1) + "M";
    if (a >= 1e4) return (n / 1e3).toFixed(0) + "k";
    if (a >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  };
  const pct = (x, d = 1) => (isFinite(x) ? (x * 100).toFixed(d) + "%" : "—");
  const sum = (a, f = (x) => x) => a.reduce((s, x) => s + (f(x) || 0), 0);
  const pad2 = (n) => String(n).padStart(2, "0");
  const hms = (ts) => {
    const d = new Date(ts * 1000);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
  };
  const hm = (ts) => {
    const d = new Date(ts * 1000);
    return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
  };
  const iso = (ts) => new Date(ts * 1000).toISOString().replace(".000", "");
  const ago = (ts, now) => {
    const s = Math.max(0, now - ts);
    if (s < 60) return `${s}s ago`;
    if (s < 3600) return `${Math.floor(s / 60)}m ago`;
    if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
    return `${Math.floor(s / 86400)}d ago`;
  };
  const icon = (name, cls = "") =>
    `<span class="inline-flex shrink-0 [&>svg]:w-4 [&>svg]:h-4 ${cls}" aria-hidden="true">${ICONS[name] || ""}</span>`;
  const tokOf = (c) =>
    c.input == null
      ? null
      : (c.input || 0) + (c.cache_create || 0) + (c.cache_read || 0) + (c.output || 0);

  // ---------------------------------------------------------------- sample data
  // Deterministic, so screenshots are stable. Every key mirrors a real struct:
  // Snapshot/Overview/Stats/PluginPage (src/web/model.rs), CallRow/SessionTotals
  // (src/store/mod.rs), doctor::Report (src/doctor.rs), log line (crates/rtok-log).
  function sampleSnapshot() {
    let seed = 42;
    const rnd = () => (seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296;
    const pick = (a) => a[Math.floor(rnd() * a.length)];
    const NOW = Math.floor(Date.UTC(2026, 8, 27, 18, 20, 0) / 1000);
    const hex = (n) =>
      Array.from({ length: n }, () => "0123456789abcdef"[Math.floor(rnd() * 16)]).join("");

    // Plugin catalogue: ids, titles, summaries, surfaces and saves_tokens are the real
    // manifests (src/plugins/*/mod.rs); enabled/fields/stats are sample.
    const cat = [
      [
        "read",
        "Read",
        "Outline, map, and search instead of dumping full files into context.",
        ["mcp", "hook"],
        true,
      ],
      [
        "cmd",
        "Bash / cmd",
        "Archive and filter command output; expand the original by id.",
        ["hook", "cli"],
        true,
      ],
      [
        "archive",
        "Archive",
        "Shrink old tool results in the live zone; pointers expand on demand.",
        ["proxy", "mcp", "cli"],
        true,
      ],
      [
        "toon",
        "TOON",
        "Compact tabular JSON in old tool results; expand returns the original.",
        ["proxy", "mcp"],
        true,
      ],
      [
        "guard",
        "Guard",
        "Deny duplicate reads and commands inside a sliding window.",
        ["hook", "cli"],
        true,
      ],
      [
        "compress",
        "Compress",
        "Extractive summaries for archived tool output; expand returns the original.",
        ["proxy"],
        true,
      ],
      [
        "graph",
        "Graph",
        "symbol / callers / impact from a tree-sitter-tags index.",
        ["mcp", "hook"],
        true,
      ],
      [
        "memory",
        "Memory",
        "Recall notes and titles without an LLM extraction step.",
        ["mcp", "hook"],
        true,
      ],
      [
        "inject",
        "Inject",
        "Budgeted SessionStart / prompt context that stays byte-stable.",
        ["hook"],
        true,
      ],
      [
        "proxy",
        "Proxy usage",
        "Record provider usage; compress mode runs plugin proxy_filter.",
        ["proxy"],
        true,
      ],
      [
        "measure",
        "Measure",
        "Only Measurement rows count as savings. Stats from transcripts and proxy usage.",
        ["cli", "proxy"],
        false,
      ],
    ];
    const enabledSet = new Set([
      "read",
      "cmd",
      "archive",
      "toon",
      "guard",
      "graph",
      "memory",
      "inject",
      "proxy",
      "measure",
    ]);
    const plugins = cat.map(([id, title, summary, surfaces, saves], i) => {
      const enabled = enabledSet.has(id);
      const before = saves && enabled ? Math.floor(20000 + (rnd() * 420000) / (i + 1)) : 0;
      const after = Math.floor(before * (0.12 + rnd() * 0.5));
      return {
        id,
        enabled,
        surfaces,
        title,
        summary,
        saves_tokens: saves,
        fields: [
          ["enabled", String(enabled)],
          ["surfaces", surfaces.join(", ")],
        ],
        stats:
          saves && enabled
            ? {
                input: Math.floor(before * 0.8),
                output: Math.floor(before * 0.05),
                cache_create: Math.floor(before * 0.1),
                cache_read: Math.floor(before * 2.4),
                est_before: before,
                est_after: after,
                rows: Math.floor(20 + rnd() * 600),
              }
            : saves
              ? {
                  input: 0,
                  output: 0,
                  cache_create: 0,
                  cache_read: 0,
                  est_before: 0,
                  est_after: 0,
                  rows: 0,
                }
              : null,
      };
    });

    const projects = ["~/GitHub/listepo/apps/rtok", "~/GitHub/listepo/apps/landing", "~/work/api"];
    const sessions = Array.from({ length: 14 }, (_, i) => {
      const started = NOW - Math.floor(600 + i * 2100 + rnd() * 900);
      const live = i < 3;
      const last = live ? NOW - Math.floor(rnd() * 120) : started + Math.floor(300 + rnd() * 1800);
      const host = i % 5 === 3 ? "codex" : i % 7 === 5 ? "cursor" : "claude";
      const inp = Math.floor(8000 + rnd() * 90000);
      return {
        id: hex(8) + "-" + hex(4) + "-" + hex(4),
        host,
        project: projects[i % 3],
        provider: host === "codex" ? "openai" : "anthropic",
        api: host === "codex" ? "responses" : "messages",
        model:
          host === "codex" ? "gpt-5-codex" : i % 4 === 0 ? "claude-opus-4-1" : "claude-sonnet-4-5",
        input: inp,
        cache_create: Math.floor(inp * (0.2 + rnd() * 0.4)),
        cache_read: Math.floor(inp * (4 + rnd() * 9)),
        output: Math.floor(inp * (0.04 + rnd() * 0.08)),
        started_at: started,
        last_activity: last,
        ended_at: live ? null : last + 5,
      };
    });

    const hookEvents = [
      "PreToolUse",
      "PostToolUse",
      "PostToolUse",
      "PreToolUse",
      "UserPromptSubmit",
      "SessionStart",
      "PreCompact",
    ];
    const mcpTools = [
      ["read", "read"],
      ["read", "search"],
      ["read", "tree"],
      ["graph", "symbol"],
      ["graph", "callers"],
      ["graph", "impact"],
      ["memory", "mem_search"],
      ["memory", "mem_get"],
      ["toon", "plan_next"],
    ];
    const ref_ids = {};
    const calls = [];
    let ts = NOW - 3;
    for (let id = 5120; calls.length < 120; id--) {
      ts -= Math.floor(2 + rnd() * rnd() * 70);
      const s = sessions[Math.floor(rnd() * rnd() * 5)];
      const r = rnd();
      let c;
      if (r < 0.46) {
        c = {
          surface: "hook",
          kind: "hook",
          plugin: null,
          name: pick(hookEvents),
          ms: +(0.4 + rnd() * 9).toFixed(1),
        };
      } else if (r < 0.64) {
        c = {
          surface: "hook",
          kind: "plugin_run",
          plugin: pick(["cmd", "guard", "read", "inject"]),
          name: null,
          ms: +(0.2 + rnd() * 4).toFixed(1),
        };
      } else if (r < 0.82) {
        const [p, n] = pick(mcpTools);
        c = {
          surface: "mcp",
          kind: "mcp_call",
          plugin: p,
          name: n,
          ms: +(1 + rnd() * 40).toFixed(1),
        };
      } else {
        c = {
          surface: "proxy",
          kind: "api_request",
          plugin: null,
          name: s.provider === "openai" ? "/v1/responses" : "/v1/messages",
          ms: +(400 + rnd() * 5200).toFixed(1),
        };
      }
      const usage = c.kind === "api_request";
      const inp = usage ? Math.floor(1500 + rnd() * 9000) : null;
      const ok = rnd() < 0.955 ? 1 : 0;
      calls.push({
        id,
        ts,
        session: s.id,
        surface: c.surface,
        kind: c.kind,
        plugin: c.plugin,
        name: c.name,
        parent_id: c.kind === "plugin_run" ? id + 1 : null,
        ms: c.ms,
        ok,
        error: ok ? null : usage ? "upstream 529 overloaded" : "timed out after 5s",
        host: usage ? s.host : null,
        provider: usage ? s.provider : null,
        model: usage ? s.model : null,
        api: usage ? s.api : null,
        input: inp,
        cache_create: usage ? Math.floor(inp * 0.3) : null,
        cache_read: usage ? Math.floor(inp * 7) : null,
        output: usage ? Math.floor(200 + rnd() * 2400) : null,
      });
      if ((c.plugin === "cmd" || c.plugin === "read" || c.kind === "api_request") && rnd() < 0.5)
        ref_ids[id] = hex(12);
    }

    const turns = Array.from({ length: 120 }, (_, i) =>
      Math.floor(9000 + i * 520 + rnd() * 6000 - (i % 23 === 0 ? 30000 : 0)),
    ).map((x) => Math.max(4000, x));
    const totals = {
      input: sum(sessions, (s) => s.input),
      output: sum(sessions, (s) => s.output),
      cache_create: sum(sessions, (s) => s.cache_create),
      cache_read: sum(sessions, (s) => s.cache_read),
      est_before: 0,
      est_after: 0,
      rows: 1840,
    };

    const logSrc = [
      ["info", "proxy", "messages", "forwarded 200 in 1.8s (in 6204 · out 911 tok)"],
      ["info", "hook", "PreToolUse", "guard allowed Read src/web/model.rs"],
      ["warn", "hook", "PreToolUse", "guard denied duplicate Read inside window (40 turns)"],
      ["info", "mcp", "read", "outline src/doctor.rs (1750 lines → 96)"],
      ["info", "plugin", "cmd", "archived 18.2 KB of cargo test output"],
      ["warn", "proxy", "messages", "upstream 529 overloaded, retrying in 2s"],
      ["error", "hook", "PostToolUse", "plugin graph panicked: index not built"],
      ["info", "plugin", "toon", "compacted tool_result 4.1 KB → 1.3 KB"],
      ["info", "mcp", "memory", "mem_search hit 3 notes"],
    ];
    const logs = Array.from({ length: 60 }, (_, i) => {
      const [lv, src, name, msg] =
        logSrc[Math.floor(rnd() * rnd() * logSrc.length * 1.4) % logSrc.length];
      const d = new Date((NOW - i * 23 - Math.floor(rnd() * 20)) * 1000);
      const st = `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())} ${pad2(d.getUTCHours())}:${pad2(d.getUTCMinutes())}:${pad2(d.getUTCSeconds())}`;
      return `${st} ${lv} ${src}/${name}: ${msg}`;
    });

    const doctor = {
      hooks_total: 7,
      hooks_by_event: {
        PostCompact: 1,
        PostToolUse: 1,
        PreCompact: 1,
        PreToolUse: 1,
        SessionEnd: 1,
        SessionStart: 1,
        UserPromptSubmit: 1,
      },
      mcp: [
        { name: "rtok", cmd: "rtok mcp", tools: 14, desc_tokens: 1180 },
        {
          name: "github",
          cmd: "npx @modelcontextprotocol/server-github",
          tools: 26,
          desc_tokens: 4310,
        },
      ],
      proxy: "localhost:8790→api.anthropic.com",
      proxy_openai: "",
      mcp_tool_search_disabled: true,
      bash_max_output_length: "30000",
      auto_compact_window: null,
      read_share: { read_tokens: 412000, grep_tokens: 61000, glob_tokens: 9000, share: 0.1452 },
      instructions: {
        rows: [
          {
            name: "CLAUDE.md",
            tokens: 3560,
            path: "~/GitHub/listepo/apps/rtok/CLAUDE.md",
            warn: true,
          },
          {
            name: "AGENTS.md",
            tokens: 1210,
            path: "~/GitHub/listepo/apps/rtok/AGENTS.md",
            warn: false,
          },
          { name: "CLAUDE.md (user)", tokens: 640, path: "~/.claude/CLAUDE.md", warn: false },
        ],
        duplicates: [["Commit in English.", ["CLAUDE.md", "AGENTS.md"]]],
      },
      skills: {
        rows: [
          {
            name: "worktrees",
            source: "user",
            desc_chars: 212,
            body_bytes: 3400,
            invocations: 9,
            warn_desc: true,
            warn_body: false,
            warn_never: false,
          },
          {
            name: "release",
            source: "project",
            desc_chars: 140,
            body_bytes: 11200,
            invocations: 0,
            warn_desc: false,
            warn_body: true,
            warn_never: true,
          },
        ],
        desc_bytes: 2860,
      },
      overlaps: [
        "host auto-compact duplicates rtok archive — set [archive] enabled = false or disable the host feature",
      ],
      tools_rewrite_advice: null,
      agents: [
        {
          host: "claude",
          kind: "cli",
          modules: [
            { name: "hooks", state: "installed", note: "" },
            { name: "mcp", state: "installed", note: "" },
            { name: "proxy", state: "installed", note: "" },
            { name: "plugin", state: "not_installed", note: "--plugin" },
          ],
        },
        {
          host: "codex",
          kind: "cli",
          modules: [
            { name: "hooks", state: "not_supported", note: "no hook API" },
            { name: "mcp", state: "installed", note: "" },
            { name: "proxy", state: "not_installed", note: "--proxy" },
            { name: "plugin", state: "not_supported", note: "" },
          ],
        },
        {
          host: "cursor",
          kind: "app",
          modules: [
            { name: "hooks", state: "installed", note: "" },
            { name: "mcp", state: "installed", note: "" },
            { name: "proxy", state: "not_supported", note: "no base URL setting" },
            { name: "plugin", state: "not_supported", note: "" },
          ],
        },
      ],
    };

    return {
      type: "snapshot",
      usage: Object.assign({}, totals, { ctt: 48213360, turns, alerts: [] }),
      plugins,
      calls,
      sessions,
      doctor,
      logs,
      skills: { header: "", rows: [] },
      ref_ids,
      stats: "",
      graph: "",
      hosts: "",
      config: "",
      services: "",
      worktrees: "",
      __now: NOW,
    };
  }

  function emptySnapshot(error) {
    return {
      type: "snapshot",
      usage: {
        input: 0,
        output: 0,
        cache_create: 0,
        cache_read: 0,
        est_before: 0,
        est_after: 0,
        rows: 0,
        ctt: 0,
        turns: [],
      },
      plugins: [],
      calls: [],
      sessions: [],
      doctor: null,
      logs: [],
      skills: { header: "", rows: [] },
      ref_ids: {},
      error: error || undefined,
      __now: Math.floor(Date.now() / 1000),
    };
  }

  // ---------------------------------------------------------------- state
  const ROUTES = [
    { id: "overview", title: "overview", sub: "usage totals, savings and health at a glance" },
    { id: "plugins", title: "plugins", sub: "catalogue, enable toggles and Measurement stats" },
    { id: "calls", title: "calls", sub: "ledger rows, newest first" },
    { id: "sessions", title: "sessions", sub: "one row per session, newest first" },
    { id: "doctor", title: "doctor", sub: "hooks, MCP servers, proxy chains, instruction audit" },
    { id: "logs", title: "logs", sub: "newest first — the lines `rtok logs` shows" },
  ];
  const S = {
    route: "overview",
    snap: null,
    source: "sample", // 'live' | 'sample'
    conn: "connecting", // 'connecting' | 'live' | 'reconnecting' | 'offline'
    preview: "live", // 'live' | 'loading' | 'empty' | 'error'
    ws: null,
    sel: { plugin: 0, call: 0, session: 0 },
    f: {
      plugins: { q: "", show: "all" },
      calls: { q: "", surface: "all", ok: "all" },
      sessions: { q: "", live: false },
      logs: { q: "", level: "all" },
    },
    expand: { ref: "", text: "", filter: "" },
    mobileCalls: 20,
  };

  // ---------------------------------------------------------------- derive
  function derive(v) {
    const u = v.usage || {};
    const plugins = v.plugins || [],
      calls = v.calls || [],
      sessions = v.sessions || [];
    const measured = plugins.filter((p) => p.stats && p.stats.est_before > 0);
    const estBefore = sum(measured, (p) => p.stats.est_before),
      estAfter = sum(measured, (p) => p.stats.est_after);
    const saved = estBefore - estAfter;
    const ctxIn = (u.input || 0) + (u.cache_create || 0) + (u.cache_read || 0);
    const now = v.__now || Math.floor(Date.now() / 1000);
    // Calls over time: 24 equal buckets over the window the frame carries.
    const tsList = calls.map((c) => c.ts);
    const t1 = tsList.length ? Math.max(...tsList) : now,
      t0 = tsList.length ? Math.min(...tsList) : now - 3600;
    const N = 24,
      span = Math.max(60, t1 - t0 + 1),
      step = span / N;
    const buckets = Array.from({ length: N }, (_, i) => ({
      t: t0 + i * step,
      hook: 0,
      mcp: 0,
      proxy: 0,
      err: 0,
    }));
    calls.forEach((c) => {
      const b = buckets[Math.min(N - 1, Math.floor((c.ts - t0) / step))];
      if (b[c.surface] != null) b[c.surface]++;
      if (!c.ok) b.err++;
    });
    const ms = calls
      .map((c) => c.ms)
      .filter((x) => x != null)
      .sort((a, b) => a - b);
    const q = (p) => (ms.length ? ms[Math.min(ms.length - 1, Math.floor(p * ms.length))] : null);
    // Sessions alive per bucket (started_at ≤ t ≤ ended_at|now).
    const liveSeries = buckets.map(
      (b) =>
        sessions.filter(
          (s) => s.started_at <= b.t + step && (s.ended_at == null || s.ended_at >= b.t),
        ).length,
    );
    const bySurface = {};
    calls.forEach((c) => {
      const o = (bySurface[c.surface] = bySurface[c.surface] || { n: 0, err: 0, last: 0 });
      o.n++;
      if (!c.ok) o.err++;
      o.last = Math.max(o.last, c.ts);
    });
    return {
      now,
      u,
      plugins,
      calls,
      sessions,
      measured,
      estBefore,
      estAfter,
      saved,
      deltaPct: estBefore ? saved / estBefore : NaN,
      cacheHit: ctxIn ? (u.cache_read || 0) / ctxIn : NaN,
      live: sessions.filter((s) => s.ended_at == null),
      enabled: plugins.filter((p) => p.enabled),
      errors: calls.filter((c) => !c.ok),
      buckets,
      step,
      liveSeries,
      bySurface,
      p50: q(0.5),
      p95: q(0.95),
      checks: doctorChecks(v.doctor),
      logs: (v.logs || []).map(parseLog),
    };
  }

  const LOG_RE = /^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d) (\S+) ([^/\s]+)\/([^:]+): (.*)$/;
  function parseLog(line, i) {
    const m = LOG_RE.exec(line);
    return m
      ? { i, raw: line, ts: m[1], level: m[2].toLowerCase(), source: m[3], name: m[4], msg: m[5] }
      : { i, raw: line, ts: "", level: "info", source: "", name: "", msg: line };
  }

  // Doctor checks derived from doctor::Report fields (src/doctor.rs:23). The Report
  // has no verdict field; each row below names the field it reads.
  function doctorChecks(d) {
    if (!d)
      return [
        {
          st: "fail",
          label: "doctor probe",
          detail: "doctor did not answer this tick — `rtok doctor` has the details",
          field: "Snapshot.doctor = null",
        },
      ];
    const c = [];
    c.push({
      st: d.hooks_total > 0 ? "pass" : "fail",
      label: "hooks installed",
      detail: `${d.hooks_total} hooks across ${Object.keys(d.hooks_by_event || {}).length} events`,
      field: "hooks_total",
    });
    c.push({
      st: (d.mcp || []).length ? "pass" : "warn",
      label: "MCP servers",
      detail: (d.mcp || []).map((s) => `${s.name} (${s.tools} tools)`).join(", ") || "none probed",
      field: "mcp[]",
    });
    c.push({
      st: d.proxy ? "pass" : "warn",
      label: "Anthropic proxy chain",
      detail: d.proxy || "ANTHROPIC_BASE_URL not set",
      field: "proxy",
    });
    c.push({
      st: d.proxy_openai ? "pass" : "skip",
      label: "OpenAI proxy chain",
      detail: d.proxy_openai || "not configured",
      field: "proxy_openai",
    });
    if (d.mcp_tool_search_disabled)
      c.push({
        st: "warn",
        label: "MCP tool search",
        detail: "likely disabled (ANTHROPIC_BASE_URL is set)",
        field: "mcp_tool_search_disabled",
      });
    c.push({
      st: d.bash_max_output_length ? "pass" : "skip",
      label: "BASH_MAX_OUTPUT_LENGTH",
      detail: d.bash_max_output_length || "(unset)",
      field: "bash_max_output_length",
    });
    c.push({
      st: d.auto_compact_window ? "pass" : "skip",
      label: "autoCompactWindow",
      detail: d.auto_compact_window || "(unset)",
      field: "auto_compact_window",
    });
    const ins = d.instructions;
    if (ins) {
      ins.rows
        .filter((r) => r.warn)
        .forEach((r) =>
          c.push({
            st: "warn",
            label: `instructions: ${r.name}`,
            detail: `${fmt(r.tokens)} tokens — ${r.path}`,
            field: "instructions.rows[].warn",
          }),
        );
      ins.duplicates.forEach(([s, names]) =>
        c.push({
          st: "warn",
          label: "duplicate instruction",
          detail: `“${s}” in ${names.join(", ")}`,
          field: "instructions.duplicates",
        }),
      );
    }
    if (d.skills) {
      const w = d.skills.rows.filter((r) => r.warn_desc || r.warn_body || r.warn_never);
      c.push({
        st: w.length ? "warn" : "pass",
        label: "skills audit",
        detail: w.length
          ? w
              .map(
                (r) =>
                  `${r.name}: ${[r.warn_desc && "long description", r.warn_body && "body > 8 KB", r.warn_never && "never invoked"].filter(Boolean).join(", ")}`,
              )
              .join(" · ")
          : `${d.skills.rows.length} skills OK`,
        field: "skills.rows[].warn_*",
      });
    }
    (d.overlaps || []).forEach((o) =>
      c.push({ st: "warn", label: "host overlap", detail: o, field: "overlaps[]" }),
    );
    if (d.tools_rewrite_advice)
      c.push({
        st: "warn",
        label: "tools rewrite",
        detail: d.tools_rewrite_advice,
        field: "tools_rewrite_advice",
      });
    return c;
  }

  // ---------------------------------------------------------------- charts (inline SVG)
  function spark(
    vals,
    { w = 96, h = 28, color = "rgb(var(--accent-fg))", area = true, label = "" } = {},
  ) {
    if (!vals || vals.length < 2)
      return `<svg width="${w}" height="${h}" aria-hidden="true"></svg>`;
    const mx = Math.max(...vals),
      mn = Math.min(...vals),
      r = mx - mn || 1;
    const pts = vals.map((v, i) => [(i / (vals.length - 1)) * w, h - 2 - ((v - mn) / r) * (h - 4)]);
    const d = pts.map((p, i) => (i ? "L" : "M") + p[0].toFixed(1) + " " + p[1].toFixed(1)).join("");
    return `<svg viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" class="overflow-visible" role="img" aria-label="${esc(label)}">
      ${area ? `<path d="${d}L${w} ${h}L0 ${h}Z" fill="${color}" opacity="0.12"/>` : ""}
      <path d="${d}" fill="none" stroke="${color}" stroke-width="1.5" stroke-linejoin="round" stroke-linecap="round"/>
      <circle cx="${last[0]}" cy="${last[1]}" r="2" fill="${color}"/></svg>`;
  }
  function miniBars(vals, { w = 96, h = 28, color = "rgb(var(--accent-fg))", label = "" } = {}) {
    if (!vals.length) return "";
    const mx = Math.max(...vals, 1),
      bw = w / vals.length;
    return `<svg viewBox="0 0 ${w} ${h}" width="${w}" height="${h}" role="img" aria-label="${esc(label)}">${vals.map((v, i) => `<rect x="${(i * bw + 1).toFixed(1)}" y="${(h - (v / mx) * h).toFixed(1)}" width="${Math.max(1, bw - 2).toFixed(1)}" height="${((v / mx) * h).toFixed(1)}" rx="1" fill="${color}" opacity="${0.45 + 0.55 * (v / mx)}"/>`).join("")}</svg>`;
  }
  const SERIES = [
    { k: "hook", label: "hook", fill: "rgb(var(--accent-fg))" },
    { k: "mcp", label: "mcp", fill: "rgb(var(--accent-fg) / 0.5)" },
    { k: "proxy", label: "proxy", fill: "rgb(var(--ink-muted) / 0.75)" },
  ];
  function callsChart(D) {
    const W = 720,
      H = 180,
      P = { l: 32, r: 8, t: 8, b: 22 };
    const b = D.buckets,
      mx = Math.max(1, ...b.map((x) => x.hook + x.mcp + x.proxy));
    const nice = Math.ceil(mx / 4) * 4,
      iw = W - P.l - P.r,
      ih = H - P.t - P.b,
      bw = iw / b.length;
    const y = (v) => P.t + ih - (v / nice) * ih;
    let g = "";
    for (let i = 0; i <= 4; i++) {
      const v = (nice / 4) * i;
      g += `<line x1="${P.l}" x2="${W - P.r}" y1="${y(v)}" y2="${y(v)}" stroke="rgb(var(--line))" stroke-dasharray="${i ? "2 3" : ""}"/><text x="${P.l - 6}" y="${y(v) + 3}" text-anchor="end" font-size="9" fill="rgb(var(--ink-subtle))">${v}</text>`;
    }
    let bars = "";
    b.forEach((x, i) => {
      let acc = 0;
      SERIES.forEach((s) => {
        const v = x[s.k];
        if (!v) return;
        bars += `<rect x="${(P.l + i * bw + 2).toFixed(1)}" y="${y(acc + v).toFixed(1)}" width="${(bw - 4).toFixed(1)}" height="${(y(acc) - y(acc + v)).toFixed(1)}" fill="${s.fill}"><title>${hm(x.t)} · ${s.label} ${v}</title></rect>`;
        acc += v;
      });
      if (x.err)
        bars += `<circle cx="${(P.l + i * bw + bw / 2).toFixed(1)}" cy="${(y(acc) - 6).toFixed(1)}" r="2.5" fill="rgb(var(--delta-fg))"><title>${x.err} failed</title></circle>`;
      if (i % 4 === 0)
        bars += `<text x="${(P.l + i * bw).toFixed(1)}" y="${H - 6}" font-size="9" fill="rgb(var(--ink-subtle))">${hm(x.t)}</text>`;
    });
    const total = sum(b, (x) => x.hook + x.mcp + x.proxy);
    return `<svg viewBox="0 0 ${W} ${H}" class="w-full h-auto" role="img" aria-label="Calls over time: ${total} calls in ${b.length} buckets of ${Math.round(D.step / 60)} minutes, ${D.errors.length} failed">${g}${bars}</svg>`;
  }
  function areaChart(vals, { h = 96, label = "" } = {}) {
    const W = 360;
    if (!vals.length) return emptyNote("ctx tokens per turn (no usage rows yet)");
    const mx = Math.max(...vals),
      pts = vals.map((v, i) => [
        (i / Math.max(1, vals.length - 1)) * W,
        h - 4 - (v / mx) * (h - 12),
      ]);
    const d = pts.map((p, i) => (i ? "L" : "M") + p[0].toFixed(1) + " " + p[1].toFixed(1)).join("");
    return `<svg viewBox="0 0 ${W} ${h}" class="w-full h-auto" preserveAspectRatio="none" role="img" aria-label="${esc(label)}">
      <defs><linearGradient id="ag" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stop-color="rgb(var(--accent-fg))" stop-opacity="0.28"/><stop offset="1" stop-color="rgb(var(--accent-fg))" stop-opacity="0"/></linearGradient></defs>
      <line x1="0" x2="${W}" y1="${h - 4}" y2="${h - 4}" stroke="rgb(var(--line))"/>
      <path d="${d}L${W} ${h}L0 ${h}Z" fill="url(#ag)"/><path d="${d}" fill="none" stroke="rgb(var(--accent-fg))" stroke-width="1.5" vector-effect="non-scaling-stroke"/></svg>`;
  }
  // Bitset mark motif: one dot per plugin, lit when enabled (echoes assets/logo.svg).
  function bitset(plugins) {
    return `<div class="grid grid-cols-6 gap-1 w-max" role="img" aria-label="${plugins.filter((p) => p.enabled).length} of ${plugins.length} plugins enabled">${plugins.map((p) => `<span title="${esc(p.id)}: ${p.enabled ? "enabled" : "disabled"}" class="w-2 h-2 rounded-full ${p.enabled ? (p.saves_tokens ? "bg-accent-fg" : "bg-accent-fg/40") : "bg-delta-fg/80"}"></span>`).join("")}</div>`;
  }

  // ---------------------------------------------------------------- UI atoms
  const panel = (title, body, { action = "", cls = "", sub = "", id = "" } = {}) => `
    <section class="glass shine flex flex-col min-w-0 ${cls}" ${id ? `aria-labelledby="${id}"` : ""}>
      <header class="flex items-center gap-2 h-11 px-3 border-b border-line/70">
        <h2 ${id ? `id="${id}"` : ""} class="text-xs font-semibold truncate">${title}</h2>
        ${sub ? `<span class="text-2xs text-ink-subtle truncate">${sub}</span>` : ""}
        <div class="ml-auto flex items-center gap-1.5 shrink-0">${action}</div>
      </header>
      <div class="min-w-0 flex-1">${body}</div>
    </section>`;
  const emptyNote = (
    text,
    extra = "",
  ) => `<div class="flex flex-col items-center justify-center gap-3 py-10 px-4 text-center">
      <div class="grid grid-cols-4 gap-1 opacity-60" aria-hidden="true">${Array.from({ length: 16 }, (_, i) => `<span class="w-1.5 h-1.5 rounded-full ${i < 6 ? "bg-accent/50" : "bg-line-strong"}"></span>`).join("")}</div>
      <p class="text-xs text-ink-muted max-w-[42ch]">${esc(text)}</p>${extra}</div>`;
  const stPill = (st) =>
    ({
      pass: '<span class="pill-ok"><span class="dot"></span>pass</span>',
      warn: '<span class="pill-warn"><span class="dot"></span>warn</span>',
      fail: '<span class="pill-fail"><span class="dot"></span>fail</span>',
      skip: '<span class="pill-muted">n/a</span>',
    })[st];
  const levelPill = (lv) =>
    lv === "error"
      ? '<span class="pill-fail">error</span>'
      : lv === "warn"
        ? '<span class="pill-warn">warn</span>'
        : lv === "debug"
          ? '<span class="pill-muted">debug</span>'
          : '<span class="pill-info">info</span>';
  const surfacePill = (s) =>
    `<span class="pill ${s === "proxy" ? "text-ink-muted border-line-strong bg-surface-2" : s === "mcp" ? "text-accent-fg border-accent/30 bg-transparent" : "text-accent-fg border-accent/40 bg-accent/10"}">${esc(s)}</span>`;
  const livePill = (s) =>
    s.ended_at == null
      ? '<span class="pill-ok"><span class="dot animate-pulse"></span>live</span>'
      : '<span class="pill-muted">ended</span>';
  const modPill = (st) =>
    st === "installed"
      ? '<span class="pill-ok" title="installed">on</span>'
      : st === "not_installed"
        ? '<span class="pill-warn" title="not installed">off</span>'
        : '<span class="pill-muted" title="not supported">n/a</span>';
  const kv = (rows) =>
    `<dl class="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4 gap-y-1.5 text-xs">${rows.map(([k, v]) => `<dt class="text-ink-subtle">${esc(k)}</dt><dd class="text-ink break-words">${v}</dd>`).join("")}</dl>`;
  const chip = (group, val, cur, label, count) =>
    `<button type="button" class="chip focus-ring" data-chip="${group}" data-val="${val}" aria-pressed="${cur === val}">${esc(label)}${count != null ? `<span class="text-ink-subtle font-normal">${count}</span>` : ""}</button>`;
  const search = (id, val, ph) =>
    `<label class="relative flex-1 min-w-[10rem] max-w-sm"><span class="sr-only">${esc(ph)}</span><input id="${id}" data-filter="${id}" type="search" class="field focus-ring pl-7" placeholder="${esc(ph)}" value="${esc(val)}" autocomplete="off" spellcheck="false"><span class="absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-subtle text-xs pointer-events-none" aria-hidden="true">⌕</span></label>`;
  const skeletonRows = (n, cols = 5) =>
    Array.from(
      { length: n },
      () =>
        `<div class="flex gap-3 px-3 py-2.5 border-b border-line/50">${Array.from({ length: cols }, (_, i) => `<div class="skeleton h-3 ${i === 1 ? "flex-1" : "w-16"}"></div>`).join("")}</div>`,
    ).join("");

  // ---------------------------------------------------------------- views
  function kpi({ label, value, sub = "", viz = "", tone = "", href = "", title = "" }) {
    const tag = href ? "a" : "div";
    return `<${tag} ${href ? `href="${href}"` : ""} title="${esc(title)}" class="glass shine focus-ring group flex flex-col gap-1.5 p-3 min-w-0 ${href ? "hover:border-line-strong transition-colors duration-fast" : ""}">
      <div class="flex items-center gap-2"><span class="kicker truncate">${label}</span>${href ? '<span class="ml-auto text-ink-subtle text-2xs opacity-0 group-hover:opacity-100 transition-opacity" aria-hidden="true">→</span>' : ""}</div>
      <div class="flex items-end justify-between gap-2 min-w-0">
        <div class="min-w-0"><div class="text-xl font-semibold leading-none truncate ${tone}">${value}</div><div class="mt-1.5 text-2xs text-ink-muted truncate">${sub}</div></div>
        <div class="shrink-0 hidden sm:block">${viz}</div>
      </div></${tag}>`;
  }

  function viewOverview(D) {
    const u = D.u;
    const top = D.measured
      .map((p) => ({ p, saved: p.stats.est_before - p.stats.est_after }))
      .sort((a, b) => b.saved - a.saved);
    const maxSaved = Math.max(1, ...top.map((x) => x.saved));
    const callSeries = D.buckets.map((b) => b.hook + b.mcp + b.proxy);
    const comp = [
      ["input", u.input, "bg-accent-fg"],
      ["cache create", u.cache_create, "bg-accent-fg/60"],
      ["cache read", u.cache_read, "bg-accent-fg/30"],
      ["output", u.output, "bg-delta"],
    ];
    const compTotal = Math.max(
      1,
      sum(comp, (c) => c[1]),
    );
    const cnt = { pass: 0, warn: 0, fail: 0 };
    D.checks.forEach((c) => {
      if (cnt[c.st] != null) cnt[c.st]++;
    });
    const agents = (S.snap.doctor && S.snap.doctor.agents) || [];
    const recentSessions = D.sessions
      .slice()
      .sort((a, b) => b.last_activity - a.last_activity)
      .slice(0, 6);

    const kpis = [
      kpi({
        label: "input tok",
        value: compact(u.input),
        sub: `ctx ${compact((u.input || 0) + (u.cache_create || 0) + (u.cache_read || 0))} incl. cache`,
        viz: spark(u.turns.slice(-40), { label: "ctx tokens per turn" }),
        title: "usage.input · sparkline: usage.turns",
      }),
      kpi({
        label: "output tok",
        value: compact(u.output),
        sub: `cache create ${compact(u.cache_create)}`,
        title: "usage.output",
      }),
      kpi({
        label: "Δ saved tok",
        value: `<span class="text-delta-fg">Δ</span> ${compact(D.saved)}`,
        sub: `est ${compact(D.estBefore)} → ${compact(D.estAfter)}`,
        viz: miniBars(
          top.slice(0, 8).map((x) => x.saved),
          { color: "rgb(var(--delta-fg))", label: "saved per plugin" },
        ),
        href: "#/plugins",
        title: "Σ plugins[].stats.est_before − est_after",
      }),
      kpi({
        label: "Δtok %",
        value: pct(D.deltaPct),
        sub: `${D.measured.length} plugins with Measurement rows`,
        tone: "text-accent-fg",
        title: "saved ÷ Σ est_before",
      }),
      kpi({
        label: "cache hit",
        value: pct(D.cacheHit),
        sub: `read ${compact(u.cache_read)} of ctx`,
        title: "usage.cache_read ÷ (input + cache_create + cache_read)",
      }),
      kpi({
        label: "calls",
        value: fmt(D.calls.length),
        sub: `${D.errors.length} failed · p95 ${D.p95 == null ? "—" : D.p95.toFixed(0) + " ms"}`,
        viz: miniBars(callSeries, { label: "calls per bucket" }),
        href: "#/calls",
        title: "calls[] (last 120 ledger rows)",
      }),
      kpi({
        label: "live sessions",
        value: `${D.live.length}<span class="text-ink-subtle text-sm"> / ${D.sessions.length}</span>`,
        sub: `${new Set(D.sessions.map((s) => s.host)).size} hosts`,
        viz: spark(D.liveSeries, { label: "sessions alive over the calls window" }),
        href: "#/sessions",
        title: "sessions[].ended_at == null",
      }),
      kpi({
        label: "plugins on",
        value: `${D.enabled.length}<span class="text-ink-subtle text-sm"> / ${D.plugins.length}</span>`,
        sub: `${D.plugins.length - D.enabled.length} disabled`,
        viz: bitset(D.plugins),
        href: "#/plugins",
        title: "plugins[].enabled",
      }),
    ].join("");

    const alerts = (u.alerts || [])
      .map(
        (a) =>
          `<div class="glass flex items-center gap-2 px-3 py-2 border-warn/50 text-xs"><span class="pill-warn">alert</span><span class="text-ink">${esc(a)}</span></div>`,
      )
      .join("");

    const plugTable = top.length
      ? `<div class="overflow-x-auto"><table class="tbl">
      <thead><tr><th scope="col">plugin</th><th scope="col" class="hidden sm:table-cell">surfaces</th><th scope="col" class="text-right">rows</th><th scope="col" class="text-right hidden md:table-cell">est before → after</th><th scope="col" class="text-right">saved</th><th scope="col" class="w-[28%]">Δ</th></tr></thead>
      <tbody>${top
        .map(
          ({
            p,
            saved,
          }) => `<tr><td><a class="row-link focus-ring rounded-sm font-semibold hover:text-accent-fg" href="#/plugins?id=${esc(p.id)}">${esc(p.id)}</a></td>
        <td class="hidden sm:table-cell text-ink-muted">${p.surfaces.map(esc).join(" · ")}</td>
        <td class="text-right text-ink-muted">${fmt(p.stats.rows)}</td>
        <td class="text-right hidden md:table-cell text-ink-muted">${compact(p.stats.est_before)} → ${compact(p.stats.est_after)}</td>
        <td class="text-right font-semibold">${compact(saved)}</td>
        <td><div class="flex items-center gap-2"><div class="flex-1 h-1.5 rounded-full bg-surface-3 overflow-hidden"><div class="h-full bg-delta-fg/80 rounded-full" style="width:${((saved / maxSaved) * 100).toFixed(1)}%"></div></div><span class="text-2xs text-ink-muted w-10 text-right">${pct(saved / p.stats.est_before, 0)}</span></div></td></tr>`,
        )
        .join("")}</tbody></table></div>`
      : emptyNote("no measured savings yet");

    const doctorBody = `<div class="p-3 flex flex-col gap-3">
      <div class="grid grid-cols-3 gap-2 text-center">
        ${[
          ["pass", cnt.pass, "text-success-fg"],
          ["warn", cnt.warn, "text-warn-fg"],
          ["fail", cnt.fail, "text-delta-fg"],
        ]
          .map(
            ([k, n, t]) =>
              `<div class="rounded-md bg-surface-2 py-2"><div class="text-lg font-semibold ${t}">${n}</div><div class="kicker">${k}</div></div>`,
          )
          .join("")}
      </div>
      <ul class="flex flex-col divide-y divide-line/60">${
        D.checks
          .filter((c) => c.st !== "pass" && c.st !== "skip")
          .slice(0, 5)
          .map(
            (c) =>
              `<li class="flex items-start gap-2 py-2"><span class="mt-0.5">${stPill(c.st)}</span><div class="min-w-0"><div class="text-xs font-semibold">${esc(c.label)}</div><div class="text-2xs text-ink-muted break-words">${esc(c.detail)}</div></div></li>`,
          )
          .join("") || `<li class="py-2 text-xs text-ink-muted">all checks pass</li>`
      }</ul></div>`;

    const sessBody = recentSessions.length
      ? `<ul class="divide-y divide-line/60">${recentSessions
          .map(
            (
              s,
            ) => `<li><a href="#/sessions?id=${esc(s.id)}" class="row-link focus-ring flex items-center gap-3 px-3 py-2 hover:bg-surface-2/70 transition-colors duration-fast">
        <div class="w-14 shrink-0">${livePill(s)}</div>
        <div class="min-w-0 flex-1"><div class="text-xs font-semibold truncate">${esc(s.id.slice(0, 8))} <span class="text-ink-muted font-normal">${esc(s.host || "-")} · ${esc(s.model || "-")}</span></div>
        <div class="text-2xs text-ink-subtle truncate">${esc(s.project || "-")}</div></div>
        <div class="text-right shrink-0"><div class="text-xs">${compact(s.input + s.cache_create + s.cache_read + s.output)}</div><div class="text-2xs text-ink-subtle">${ago(s.last_activity, D.now)}</div></div></a></li>`,
          )
          .join("")}</ul>`
      : emptyNote("no sessions yet");

    const surfaceRow = (k, label) => {
      const o = D.bySurface[k];
      return `<div class="flex items-center gap-2 text-2xs"><span class="w-12 text-ink-muted">${label}</span><span class="text-ink">${o ? o.n : 0} calls</span>${o && o.err ? `<span class="text-delta-fg">${o.err} failed</span>` : ""}<span class="ml-auto text-ink-subtle">${o ? "last " + ago(o.last, D.now) : "no activity"}</span></div>`;
    };
    const d = S.snap.doctor;
    const integBody = d
      ? `<div class="p-3 flex flex-col gap-3">
      <div class="overflow-x-auto"><table class="tbl"><thead><tr><th scope="col">host</th>${["hooks", "mcp", "proxy", "plugin"].map((m) => `<th scope="col">${m}</th>`).join("")}</tr></thead>
      <tbody>${agents
        .map(
          (a) =>
            `<tr><td class="font-semibold">${esc(a.host)} <span class="text-ink-subtle font-normal">${esc(a.kind)}</span></td>${[
              "hooks",
              "mcp",
              "proxy",
              "plugin",
            ]
              .map((m) => {
                const r = a.modules.find((x) => x.name === m);
                return `<td title="${esc(r && r.note)}">${r ? modPill(r.state) : "—"}</td>`;
              })
              .join("")}</tr>`,
        )
        .join("")}</tbody></table></div>
      <div class="flex flex-col gap-1.5 pt-1 border-t border-line/60">${surfaceRow("hook", "hook")}${surfaceRow("mcp", "mcp")}${surfaceRow("proxy", "proxy")}</div>
      <div class="text-2xs text-ink-muted break-words"><span class="text-ink-subtle">proxy</span> ${esc(d.proxy || "—")}</div></div>`
      : emptyNote("doctor did not answer this tick — `rtok doctor` has the details");

    const logsBody = D.logs.length
      ? `<ol class="font-mono text-2xs divide-y divide-line/40">${D.logs
          .slice(0, 9)
          .map(
            (l) =>
              `<li class="flex items-baseline gap-2 px-3 py-1.5 min-w-0"><span class="text-ink-subtle shrink-0">${esc(l.ts.slice(11))}</span><span class="shrink-0 w-12">${levelPill(l.level)}</span><span class="truncate"><span class="text-ink-muted">${esc(l.source)}/${esc(l.name)}:</span> ${esc(l.msg)}</span></li>`,
          )
          .join("")}</ol>`
      : emptyNote("no logs yet");

    return `
      <div class="flex flex-col gap-3">
        ${alerts}
        <div class="grid grid-cols-2 sm:grid-cols-4 2xl:grid-cols-8 gap-2 md:gap-3">${kpis}</div>
        <div class="grid grid-cols-1 xl:grid-cols-12 gap-3">
          ${panel(
            "calls over time",
            `<div class="p-3">${D.calls.length ? callsChart(D) : emptyNote("no calls yet (the ledger fills as hooks, MCP and the proxy run)")}
            <div class="flex flex-wrap items-center gap-x-4 gap-y-1 mt-2 text-2xs text-ink-muted">${SERIES.map((s) => `<span class="inline-flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-sm" style="background:${s.fill}"></span>${s.label} ${D.bySurface[s.k] ? D.bySurface[s.k].n : 0}</span>`).join("")}<span class="inline-flex items-center gap-1.5"><span class="w-2 h-2 rounded-full bg-delta-fg"></span>failed ${D.errors.length}</span><span class="ml-auto">p50 ${D.p50 == null ? "—" : D.p50.toFixed(1)} ms · p95 ${D.p95 == null ? "—" : D.p95.toFixed(0)} ms</span></div></div>`,
            {
              cls: "xl:col-span-8",
              sub: `${D.calls.length} rows · ${Math.max(1, Math.round(D.step / 60))} min buckets`,
              action: '<a href="#/calls" class="btn btn-ghost">calls →</a>',
              id: "h-cot",
            },
          )}
          ${panel(
            "tokens",
            `<div class="p-3 flex flex-col gap-3">
            <div><div class="flex h-2.5 rounded-full overflow-hidden bg-surface-3" role="img" aria-label="token composition">${comp.map(([, v, c]) => `<div class="${c}" style="width:${((v / compTotal) * 100).toFixed(2)}%"></div>`).join("")}</div>
            <div class="grid grid-cols-2 gap-x-3 gap-y-1 mt-2 text-2xs">${comp.map(([k, v, c]) => `<div class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-sm ${c}"></span><span class="text-ink-muted">${k}</span><span class="ml-auto text-ink">${compact(v)}</span></div>`).join("")}</div></div>
            <div><div class="flex items-baseline justify-between"><span class="kicker">ctx per turn</span><span class="text-2xs text-ink-subtle">last ${u.turns.length} turns</span></div>${areaChart(u.turns, { label: "context tokens per turn" })}</div>
            <div class="flex items-baseline justify-between text-xs"><span class="text-ink-muted" title="Σ ctx × turns-after">ctt</span><span class="font-semibold">${fmt(u.ctt)}</span></div></div>`,
            { cls: "xl:col-span-4", sub: "usage rows, all apis", id: "h-tok" },
          )}
          ${panel("top plugins by savings", plugTable, { cls: "xl:col-span-7", sub: "Measurement rows", id: "h-top" })}
          ${panel("doctor", doctorBody, { cls: "xl:col-span-5", action: '<a href="#/doctor" class="btn btn-ghost">doctor →</a>', id: "h-doc" })}
          ${panel("recent sessions", sessBody, { cls: "xl:col-span-4", sub: `${D.live.length} live`, action: '<a href="#/sessions" class="btn btn-ghost">all →</a>', id: "h-ses" })}
          ${panel("integrations", integBody, { cls: "xl:col-span-4", sub: "hooks · MCP · proxy", id: "h-int" })}
          ${panel("logs", logsBody, { cls: "xl:col-span-4", sub: "tail", action: '<a href="#/logs" class="btn btn-ghost">logs →</a>', id: "h-log" })}
        </div>
      </div>`;
  }

  const toolbar = (inner) =>
    `<div class="glass flex flex-wrap items-center gap-2 p-2">${inner}</div>`;
  const noMatch = (what) =>
    emptyNote(
      `no ${what} match these filters`,
      '<button type="button" class="btn" data-action="clear-filters">clear filters</button>',
    );
  const split = (list, detail) =>
    `<div class="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(320px,380px)] 2xl:grid-cols-[minmax(0,1fr)_460px] gap-3 items-start">${list}<div id="detail" class="lg:sticky lg:top-[5.25rem] min-w-0">${detail}</div></div>`;

  function statsWidget(st) {
    // Same six cells as TokenStatsWidget (crates/rtok-webui/ui/app.slint:92).
    const cells = [
      ["input", fmt(st.input)],
      ["output", fmt(st.output)],
      ["cache read", fmt(st.cache_read)],
      ["cache create", fmt(st.cache_create)],
      ["est before → after", `${fmt(st.est_before)} → ${fmt(st.est_after)}`, true],
      ["rows", fmt(st.rows)],
    ];
    return `<div class="grid grid-cols-2 sm:grid-cols-3 gap-px rounded-md overflow-hidden border border-line bg-line">${cells.map(([k, v, a]) => `<div class="bg-surface p-2.5 min-w-0"><div class="text-2xs text-ink-subtle truncate">${k}</div><div class="text-sm font-semibold truncate ${a ? "text-accent-fg" : ""}">${v}</div></div>`).join("")}</div>`;
  }

  // -- plugins
  function viewPlugins(D) {
    const f = S.f.plugins,
      qq = f.q.toLowerCase();
    const rows = D.plugins
      .map((p, i) => ({ p, i }))
      .filter(
        ({ p }) =>
          (f.show === "all" ||
            (f.show === "on" && p.enabled) ||
            (f.show === "off" && !p.enabled) ||
            (f.show === "saves" && p.saves_tokens)) &&
          (!qq || (p.id + " " + p.title + " " + p.summary).toLowerCase().includes(qq)),
      );
    const sel = D.plugins[S.sel.plugin] || D.plugins[0];
    const saved = (p) => (p.stats ? p.stats.est_before - p.stats.est_after : null);
    const sw = (p) =>
      `<button type="button" class="switch focus-ring" role="switch" aria-checked="${p.enabled}" aria-label="toggle ${esc(p.id)}" data-toggle="${esc(p.id)}"></button>`;
    const bar = toolbar(`${search("q-plugins", f.q, "filter plugins")}
      <div class="flex flex-wrap gap-1.5" role="group" aria-label="Show">${chip("plugins.show", "all", f.show, "all", D.plugins.length)}${chip("plugins.show", "on", f.show, "enabled", D.enabled.length)}${chip("plugins.show", "off", f.show, "disabled", D.plugins.length - D.enabled.length)}${chip("plugins.show", "saves", f.show, "saves tokens", D.plugins.filter((p) => p.saves_tokens).length)}</div>
      <span class="ml-auto text-2xs text-ink-subtle">${rows.length} shown</span>`);
    if (!D.plugins.length)
      return `<div class="flex flex-col gap-3">${bar}${panel("plugins", emptyNote("no plugins in this frame"))}</div>`;
    const table = rows.length
      ? `<div class="hidden md:block overflow-x-auto"><table class="tbl" role="grid" aria-label="plugins">
      <thead><tr><th scope="col" class="w-12">on</th><th scope="col">plugin</th><th scope="col" class="hidden xl:table-cell">surfaces</th><th scope="col" class="text-right">rows</th><th scope="col" class="text-right hidden 2xl:table-cell">est before</th><th scope="col" class="text-right hidden 2xl:table-cell">est after</th><th scope="col" class="text-right">saved</th><th scope="col" class="text-right">Δ%</th></tr></thead>
      <tbody>${rows
        .map(
          ({
            p,
            i,
          }) => `<tr data-select="plugin" data-i="${i}" aria-selected="${sel === p}" class="cursor-pointer ${p.enabled ? "" : "text-ink-subtle"}">
        <td>${sw(p)}</td>
        <td class="max-w-[22rem]"><button type="button" class="row-link focus-ring rounded-sm text-left" data-select="plugin" data-i="${i}"><span class="font-semibold ${sel === p ? "text-accent-fg" : ""}">${esc(p.title)}</span> <span class="text-ink-subtle">${esc(p.id)}</span></button><div class="text-2xs text-ink-muted truncate">${esc(p.summary)}</div></td>
        <td class="hidden xl:table-cell">${p.surfaces.map((s) => surfacePill(s)).join(" ")}</td>
        <td class="text-right">${p.stats ? fmt(p.stats.rows) : "—"}</td>
        <td class="text-right hidden 2xl:table-cell text-ink-muted">${p.stats ? compact(p.stats.est_before) : "—"}</td>
        <td class="text-right hidden 2xl:table-cell text-ink-muted">${p.stats ? compact(p.stats.est_after) : "—"}</td>
        <td class="text-right font-semibold">${p.stats && saved(p) > 0 ? compact(saved(p)) : "—"}</td>
        <td class="text-right text-ink-muted">${p.stats && p.stats.est_before ? pct(saved(p) / p.stats.est_before, 0) : "—"}</td></tr>`,
        )
        .join("")}</tbody></table></div>
      <ul class="md:hidden divide-y divide-line/60">${rows
        .map(
          ({
            p,
            i,
          }) => `<li class="flex items-center gap-3 px-3 py-2.5 ${sel === p ? "bg-accent/10" : ""}">
        <button type="button" class="row-link focus-ring flex-1 min-w-0 text-left min-h-[44px]" data-select="plugin" data-i="${i}"><div class="text-sm font-semibold ${p.enabled ? "" : "text-ink-subtle"}">${esc(p.title)} <span class="text-2xs text-ink-subtle font-normal">${esc(p.id)}</span></div>
        <div class="text-2xs text-ink-muted">${p.surfaces.join(" · ")}${p.stats && saved(p) > 0 ? ` · <span class="text-ink">Δ ${compact(saved(p))}</span>` : ""}</div></button>${sw(p)}</li>`,
        )
        .join("")}</ul>`
      : noMatch("plugins");
    const detail = sel
      ? panel(
          esc(sel.title),
          `<div class="p-3 flex flex-col gap-3">
        <div class="flex items-center gap-3"><button type="button" class="switch focus-ring" role="switch" aria-checked="${sel.enabled}" aria-labelledby="lbl-en" data-toggle="${esc(sel.id)}"></button><span id="lbl-en" class="text-xs">${sel.enabled ? "enabled" : "disabled"}</span><span class="ml-auto flex gap-1">${sel.surfaces.map(surfacePill).join("")}</span></div>
        <p class="text-xs text-ink-muted">${esc(sel.summary)}</p>
        ${sel.fields.length ? kv(sel.fields.map(([k, v]) => [k, esc(v)])) : ""}
        ${sel.saves_tokens && sel.stats ? statsWidget(sel.stats) : `<p class="text-2xs text-ink-subtle">${sel.saves_tokens ? "no Measurement rows yet" : "does not record Measurement rows"}</p>`}
        <p class="text-2xs text-ink-subtle">toggle sends <code class="text-ink-muted">{"set":{"key":"plugins.${esc(sel.id)}.enabled"}}</code></p></div>`,
          { sub: esc(sel.id), id: "h-pd" },
        )
      : "";
    return `<div class="flex flex-col gap-3">${bar}${split(panel("plugins", table, { sub: "catalogue", id: "h-pl" }), detail)}</div>`;
  }

  // -- calls
  function viewCalls(D) {
    const f = S.f.calls,
      qq = f.q.toLowerCase();
    const rows = D.calls
      .map((c, i) => ({ c, i }))
      .filter(
        ({ c }) =>
          (f.surface === "all" || c.surface === f.surface) &&
          (f.ok === "all" || (f.ok === "err" ? !c.ok : c.ok)) &&
          (!qq ||
            [c.name, c.plugin, c.session, c.kind, c.model].join(" ").toLowerCase().includes(qq)),
      );
    const sel = D.calls[S.sel.call];
    const count = (k) => D.calls.filter((c) => c.surface === k).length;
    const bar = toolbar(`${search("q-calls", f.q, "filter calls")}
      <div class="flex flex-wrap gap-1.5" role="group" aria-label="Surface">${chip("calls.surface", "all", f.surface, "all", D.calls.length)}${["hook", "mcp", "proxy"].map((k) => chip("calls.surface", k, f.surface, k, count(k))).join("")}</div>
      <div class="flex flex-wrap gap-1.5" role="group" aria-label="Result">${chip("calls.ok", "all", f.ok, "any")}${chip("calls.ok", "ok", f.ok, "ok")}${chip("calls.ok", "err", f.ok, "failed", D.errors.length)}</div>
      <span class="ml-auto text-2xs text-ink-subtle">${rows.length} of ${D.calls.length}</span>`);
    if (!D.calls.length)
      return `<div class="flex flex-col gap-3">${bar}${panel("calls (newest first)", emptyNote("no calls yet (the ledger fills as hooks, MCP and the proxy run)"))}</div>`;
    const ref = (c) => (S.snap.ref_ids || {})[c.id] || "";
    const table = rows.length
      ? `<div class="hidden md:block overflow-auto max-h-[calc(100vh-13rem)]"><table class="tbl" aria-label="calls">
      <thead><tr><th scope="col">time</th><th scope="col">surface</th><th scope="col">kind</th><th scope="col">name</th><th scope="col" class="hidden xl:table-cell">plugin</th><th scope="col" class="hidden xl:table-cell">session</th><th scope="col" class="text-right">ms</th><th scope="col" class="text-right">tokens</th><th scope="col">ok</th></tr></thead>
      <tbody>${rows
        .map(
          ({
            c,
            i,
          }) => `<tr data-select="call" data-i="${i}" aria-selected="${S.sel.call === i}" class="cursor-pointer">
        <td class="text-ink-muted" title="${iso(c.ts)}">${hms(c.ts)}</td><td>${surfacePill(c.surface)}</td><td class="text-ink-muted">${esc(c.kind)}</td>
        <td class="max-w-[16rem] truncate"><button type="button" class="row-link focus-ring rounded-sm ${S.sel.call === i ? "text-accent-fg font-semibold" : ""}" data-select="call" data-i="${i}">${esc(c.name || "-")}</button>${ref(c) ? ' <span class="pill-muted" title="archived — expandable">ref</span>' : ""}</td>
        <td class="hidden xl:table-cell text-ink-muted">${esc(c.plugin || "-")}</td><td class="hidden xl:table-cell text-ink-subtle">${esc(c.session.slice(0, 8))}</td>
        <td class="text-right ${c.ms > 3000 ? "text-warn-fg" : ""}">${c.ms == null ? "-" : c.ms.toFixed(1)}</td><td class="text-right">${tokOf(c) == null ? '<span class="text-ink-subtle">-</span>' : compact(tokOf(c))}</td>
        <td>${c.ok ? '<span class="text-success-fg" aria-label="ok">✓</span>' : '<span class="pill-fail">fail</span>'}</td></tr>`,
        )
        .join("")}</tbody></table></div>
      <ul class="md:hidden divide-y divide-line/60">${rows
        .slice(0, S.mobileCalls)
        .map(
          ({
            c,
            i,
          }) => `<li><button type="button" class="row-link focus-ring w-full text-left px-3 py-2.5 min-h-[44px] ${S.sel.call === i ? "bg-accent/10" : ""}" data-select="call" data-i="${i}">
        <div class="flex items-center gap-2 text-xs"><span class="text-ink-muted">${hms(c.ts)}</span>${surfacePill(c.surface)}<span class="text-ink-muted">${esc(c.kind)}</span><span class="ml-auto">${c.ok ? '<span class="text-success-fg">✓</span>' : '<span class="pill-fail">fail</span>'}</span></div>
        <div class="mt-1 text-sm font-semibold truncate">${esc(c.name || c.plugin || "-")}</div>
        <div class="text-2xs text-ink-subtle">${esc(c.session.slice(0, 8))} · ${c.ms == null ? "-" : c.ms.toFixed(1)} ms · ${tokOf(c) == null ? "-" : fmt(tokOf(c)) + " tok"}</div></button></li>`,
        )
        .join(
          "",
        )}</ul>${rows.length > S.mobileCalls ? `<div class="md:hidden p-2"><button type="button" class="btn w-full" data-action="more-calls">show ${Math.min(20, rows.length - S.mobileCalls)} more of ${rows.length - S.mobileCalls}</button></div>` : ""}`
      : noMatch("calls");
    let detail = "";
    if (sel) {
      const r = ref(sel);
      const exp = S.expand.ref === r && r ? S.expand : null;
      const lines = exp
        ? exp.text
            .split("\n")
            .filter((l) => !exp.filter || l.toLowerCase().includes(exp.filter.toLowerCase()))
        : [];
      detail = panel(
        "detail",
        `<div class="p-3 flex flex-col gap-3">
        <div class="flex items-center gap-2">${surfacePill(sel.surface)}<span class="text-sm font-semibold truncate">${esc(sel.name || "-")}</span><span class="ml-auto">${sel.ok ? '<span class="pill-ok">ok</span>' : '<span class="pill-fail">failed</span>'}</span></div>
        ${sel.error ? `<div class="rounded-md border border-delta/40 bg-delta/10 px-2.5 py-2 text-xs text-delta-fg" role="alert">${esc(sel.error)}</div>` : ""}
        ${kv([
          ["time", `${hms(sel.ts)} <span class="text-ink-subtle">${iso(sel.ts)}</span>`],
          ["session", esc(sel.session)],
          ["kind", esc(sel.kind)],
          ["plugin", esc(sel.plugin || "-")],
          ["host", esc(sel.host || "-")],
          ["provider", esc(sel.provider || "-")],
          ["model", esc(sel.model || "-")],
          ["api", esc(sel.api || "-")],
          ["ms", sel.ms == null ? "-" : sel.ms.toFixed(1)],
          [
            "tokens",
            tokOf(sel) == null
              ? "-"
              : `${fmt(tokOf(sel))} <span class="text-ink-subtle">in ${fmt(sel.input)} · c+ ${fmt(sel.cache_create)} · cr ${fmt(sel.cache_read)} · out ${fmt(sel.output)}</span>`,
          ],
          ["parent", sel.parent_id == null ? "-" : "#" + sel.parent_id],
          ["ref_id", r ? esc(r) : "-"],
        ])}
        ${
          r
            ? `<div class="flex flex-col gap-2 pt-2 border-t border-line/60">
          <div class="flex items-center gap-2"><button type="button" class="btn btn-primary" data-expand="${esc(r)}">expand ${esc(r)}</button>${exp ? `<span class="text-2xs text-ink-subtle">${lines.length} lines</span>` : ""}</div>
          ${
            exp
              ? `<input type="search" class="field focus-ring" placeholder="filter" aria-label="filter expanded output" data-filter="q-expand" value="${esc(exp.filter)}">
          <pre class="max-h-72 overflow-auto rounded-md bg-canvas/70 border border-line p-2.5 text-2xs leading-relaxed text-ink-muted whitespace-pre-wrap break-words">${esc(lines.join("\n")) || '<span class="text-ink-subtle">no lines match</span>'}</pre>`
              : ""
          }</div>`
            : ""
        }
      </div>`,
        { sub: "#" + sel.id, id: "h-cd" },
      );
    }
    return `<div class="flex flex-col gap-3">${bar}${split(panel("calls (newest first)", table, { sub: "last 120 ledger rows", id: "h-cl" }), detail)}</div>`;
  }

  // -- sessions
  function viewSessions(D) {
    const f = S.f.sessions,
      qq = f.q.toLowerCase();
    const rows = D.sessions
      .map((s, i) => ({ s, i }))
      .filter(
        ({ s }) =>
          (!f.live || s.ended_at == null) &&
          (!qq ||
            [s.id, s.host, s.model, s.project, s.provider].join(" ").toLowerCase().includes(qq)),
      );
    const sel = D.sessions[S.sel.session];
    const tot = (s) => s.input + s.cache_create + s.cache_read + s.output;
    const bar = toolbar(`${search("q-sessions", f.q, "filter sessions")}
      <div class="flex items-center gap-2"><button type="button" class="switch focus-ring" role="switch" aria-checked="${f.live}" aria-labelledby="lbl-live" data-action="live-only"></button><span id="lbl-live" class="text-xs">live only</span></div>
      <span class="ml-auto text-2xs text-ink-subtle">${D.live.length} live · ${D.sessions.length} total</span>`);
    if (!D.sessions.length)
      return `<div class="flex flex-col gap-3">${bar}${panel("sessions", emptyNote("no sessions yet"))}</div>`;
    const table = rows.length
      ? `<div class="hidden md:block overflow-x-auto"><table class="tbl" aria-label="sessions">
      <thead><tr><th scope="col">status</th><th scope="col">session</th><th scope="col">host</th><th scope="col" class="hidden xl:table-cell">model</th><th scope="col" class="text-right">input</th><th scope="col" class="text-right hidden 2xl:table-cell">cache +</th><th scope="col" class="text-right hidden 2xl:table-cell">cache r</th><th scope="col" class="text-right">output</th><th scope="col" class="text-right">last activity</th></tr></thead>
      <tbody>${rows
        .map(
          ({
            s,
            i,
          }) => `<tr data-select="session" data-i="${i}" aria-selected="${S.sel.session === i}" class="cursor-pointer">
        <td>${livePill(s)}</td><td><button type="button" class="row-link focus-ring rounded-sm font-semibold ${S.sel.session === i ? "text-accent-fg" : ""}" data-select="session" data-i="${i}">${esc(s.id.slice(0, 13))}</button></td>
        <td class="text-ink-muted">${esc(s.host || "-")}</td><td class="hidden xl:table-cell text-ink-muted">${esc(s.model || "-")}</td>
        <td class="text-right">${compact(s.input)}</td><td class="text-right hidden 2xl:table-cell text-ink-muted">${compact(s.cache_create)}</td><td class="text-right hidden 2xl:table-cell text-ink-muted">${compact(s.cache_read)}</td><td class="text-right">${compact(s.output)}</td>
        <td class="text-right text-ink-muted" title="${iso(s.last_activity)}">${ago(s.last_activity, D.now)}</td></tr>`,
        )
        .join("")}</tbody></table></div>
      <ul class="md:hidden divide-y divide-line/60">${rows
        .map(
          ({
            s,
            i,
          }) => `<li><button type="button" class="row-link focus-ring w-full text-left px-3 py-2.5 min-h-[44px] ${S.sel.session === i ? "bg-accent/10" : ""}" data-select="session" data-i="${i}">
        <div class="flex items-center gap-2">${livePill(s)}<span class="text-sm font-semibold">${esc(s.id.slice(0, 8))}</span><span class="ml-auto text-2xs text-ink-subtle">${ago(s.last_activity, D.now)}</span></div>
        <div class="mt-1 text-2xs text-ink-muted">${esc(s.host || "-")} · ${esc(s.model || "-")} · ${compact(tot(s))} tok</div></button></li>`,
        )
        .join("")}</ul>`
      : noMatch("sessions");
    let detail = "";
    if (sel) {
      const sc = D.calls.filter((c) => c.session === sel.id);
      const parts = [
        ["input", sel.input, "bg-accent-fg"],
        ["cache create", sel.cache_create, "bg-accent-fg/60"],
        ["cache read", sel.cache_read, "bg-accent-fg/30"],
        ["output", sel.output, "bg-delta"],
      ];
      const T = Math.max(1, tot(sel));
      detail = panel(
        "detail",
        `<div class="p-3 flex flex-col gap-3">
        <div class="flex items-center gap-2">${livePill(sel)}<span class="text-sm font-semibold break-all">${esc(sel.id)}</span></div>
        ${kv([
          ["host", esc(sel.host || "-")],
          ["project", esc(sel.project || "-")],
          ["provider", esc(sel.provider || "-")],
          ["api", esc(sel.api || "-")],
          ["model", esc(sel.model || "-")],
          [
            "started",
            `${hms(sel.started_at)} <span class="text-ink-subtle">${ago(sel.started_at, D.now)}</span>`,
          ],
          ["last", hms(sel.last_activity)],
          [
            "ended",
            sel.ended_at == null ? '<span class="text-success-fg">live</span>' : hms(sel.ended_at),
          ],
        ])}
        <div><div class="flex h-2 rounded-full overflow-hidden bg-surface-3">${parts.map(([, v, c]) => `<div class="${c}" style="width:${((v / T) * 100).toFixed(2)}%"></div>`).join("")}</div>
        <div class="grid grid-cols-2 gap-x-3 gap-y-1 mt-2 text-2xs">${parts.map(([k, v, c]) => `<div class="flex items-center gap-1.5"><span class="w-2 h-2 rounded-sm ${c}"></span><span class="text-ink-muted">${k}</span><span class="ml-auto">${fmt(v)}</span></div>`).join("")}</div></div>
        <div class="pt-2 border-t border-line/60"><div class="kicker mb-1.5">calls ${sc.length}</div>
        ${
          sc.length
            ? `<ul class="text-2xs divide-y divide-line/40 max-h-64 overflow-auto">${sc
                .slice(0, 40)
                .map(
                  (c) =>
                    `<li class="flex items-center gap-2 py-1"><span class="text-ink-subtle">${hms(c.ts)}</span>${surfacePill(c.surface)}<span class="text-ink-muted">${esc(c.kind)}</span><span class="truncate">${esc(c.name || c.plugin || "-")}</span>${c.ok ? "" : '<span class="pill-fail ml-auto">fail</span>'}</li>`,
                )
                .join("")}</ul>`
            : '<p class="text-2xs text-ink-subtle">no calls from this session in the current frame</p>'
        }</div>
      </div>`,
        { id: "h-sd" },
      );
    }
    return `<div class="flex flex-col gap-3">${bar}${split(panel("sessions", table, { sub: "newest first", id: "h-sl" }), detail)}</div>`;
  }

  // -- doctor
  function viewDoctor(D) {
    const d = S.snap.doctor;
    const cnt = { pass: 0, warn: 0, fail: 0, skip: 0 };
    D.checks.forEach((c) => cnt[c.st]++);
    const summary = `<div class="grid grid-cols-2 sm:grid-cols-4 gap-2 md:gap-3">${[
      ["pass", "text-success-fg"],
      ["warn", "text-warn-fg"],
      ["fail", "text-delta-fg"],
      ["skip", "text-ink-muted"],
    ]
      .map(
        ([k, t]) =>
          `<div class="glass shine p-3"><div class="kicker">${k === "skip" ? "not set" : k}</div><div class="text-xl font-semibold ${t}">${cnt[k]}</div></div>`,
      )
      .join("")}</div>`;
    const checks = panel(
      "checks",
      `<ul class="divide-y divide-line/60">${D.checks.map((c) => `<li class="flex items-start gap-3 px-3 py-2.5"><span class="w-12 shrink-0 mt-0.5">${stPill(c.st)}</span><div class="min-w-0 flex-1"><div class="text-xs font-semibold">${esc(c.label)}</div><div class="text-2xs text-ink-muted break-words">${esc(c.detail)}</div></div><code class="hidden sm:block text-2xs text-ink-subtle shrink-0">${esc(c.field)}</code></li>`).join("")}</ul>`,
      { sub: "derived from doctor::Report fields", id: "h-chk" },
    );
    if (!d) return `<div class="flex flex-col gap-3">${summary}${checks}</div>`;
    const ev = Object.entries(d.hooks_by_event || {}),
      mxE = Math.max(1, ...ev.map((e) => e[1]));
    const hops = (chain) =>
      chain
        ? chain
            .split("→")
            .map(
              (h, i, a) =>
                `<span class="pill-info">${esc(h.trim())}</span>${i < a.length - 1 ? '<span class="text-ink-subtle" aria-label="to">→</span>' : ""}`,
            )
            .join(" ")
        : '<span class="text-ink-subtle text-xs">not set</span>';
    const rs = d.read_share;
    return `<div class="flex flex-col gap-3">${summary}
      <div class="grid grid-cols-1 xl:grid-cols-12 gap-3">
        ${checks.replace('class="glass shine flex', 'class="xl:col-span-7 glass shine flex')}
        <div class="xl:col-span-5 flex flex-col gap-3">
          ${panel("hooks", `<div class="p-3 flex flex-col gap-1.5">${ev.map(([k, n]) => `<div class="flex items-center gap-2 text-xs"><span class="w-36 truncate text-ink-muted">${esc(k)}</span><div class="flex-1 h-1.5 rounded-full bg-surface-3"><div class="h-full rounded-full bg-accent-fg" style="width:${(n / mxE) * 100}%"></div></div><span class="w-6 text-right">${n}</span></div>`).join("") || '<p class="text-xs text-ink-muted">no hooks installed</p>'}</div>`, { sub: `${d.hooks_total} total`, id: "h-hk" })}
          ${panel("proxy chains", `<div class="p-3 flex flex-col gap-2 text-xs"><div class="flex flex-wrap items-center gap-1.5"><span class="w-16 text-ink-subtle">anthropic</span>${hops(d.proxy)}</div><div class="flex flex-wrap items-center gap-1.5"><span class="w-16 text-ink-subtle">openai</span>${hops(d.proxy_openai)}</div>${d.mcp_tool_search_disabled ? '<p class="text-2xs text-warn-fg">mcp_tool_search likely disabled (ANTHROPIC_BASE_URL is set)</p>' : ""}</div>`, { id: "h-px" })}
        </div>
        ${panel("MCP servers", `<div class="overflow-x-auto"><table class="tbl"><thead><tr><th scope="col">name</th><th scope="col" class="text-right">tools</th><th scope="col" class="text-right">desc tokens</th><th scope="col">cmd</th></tr></thead><tbody>${d.mcp.map((s) => `<tr><td class="font-semibold">${esc(s.name)}</td><td class="text-right">${s.tools}</td><td class="text-right">~${fmt(s.desc_tokens)}</td><td class="text-ink-muted truncate max-w-[18rem]">${esc(s.cmd)}</td></tr>`).join("")}</tbody></table></div>`, { cls: "xl:col-span-7", sub: `${d.mcp.length} probed`, id: "h-mcp" })}
        ${panel(
          "environment",
          `<div class="p-3">${kv([
            ["BASH_MAX_OUTPUT_LENGTH", esc(d.bash_max_output_length || "(unset)")],
            ["autoCompactWindow", esc(d.auto_compact_window || "(unset)")],
            [
              "read share",
              rs
                ? `${pct(rs.share)} <span class="text-ink-subtle">grep ${compact(rs.grep_tokens)} · glob ${compact(rs.glob_tokens)} · read ${compact(rs.read_tokens)}</span>`
                : "—",
            ],
            [
              "skills desc bytes",
              d.skills
                ? `${fmt(d.skills.desc_bytes)} <span class="text-ink-subtle">≈ ${fmt(Math.round(d.skills.desc_bytes / 4))} tok per request</span>`
                : "—",
            ],
          ])}</div>`,
          { cls: "xl:col-span-5", id: "h-env" },
        )}
        ${d.instructions ? panel("instructions", `<div class="overflow-x-auto"><table class="tbl"><thead><tr><th scope="col">file</th><th scope="col" class="text-right">tokens</th><th scope="col">path</th><th scope="col"></th></tr></thead><tbody>${d.instructions.rows.map((r) => `<tr><td class="font-semibold">${esc(r.name)}</td><td class="text-right">${fmt(r.tokens)}</td><td class="text-ink-muted truncate max-w-[20rem]">${esc(r.path)}</td><td>${r.warn ? '<span class="pill-warn">WARN</span>' : ""}</td></tr>`).join("")}</tbody></table></div>${d.instructions.duplicates.map(([s, n]) => `<p class="px-3 py-2 text-2xs text-warn-fg border-t border-line/60">duplicate “${esc(s)}” in ${esc(n.join(", "))}</p>`).join("")}`, { cls: "xl:col-span-7", id: "h-ins" }) : ""}
        ${panel(
          "agents × modules",
          `<div class="overflow-x-auto"><table class="tbl"><thead><tr><th scope="col">host</th>${["hooks", "mcp", "proxy", "plugin"].map((m) => `<th scope="col">${m}</th>`).join("")}</tr></thead><tbody>${d.agents
            .map(
              (a) =>
                `<tr><td class="font-semibold">${esc(a.host)} <span class="text-ink-subtle font-normal">${esc(a.kind)}</span></td>${[
                  "hooks",
                  "mcp",
                  "proxy",
                  "plugin",
                ]
                  .map((m) => {
                    const r = a.modules.find((x) => x.name === m);
                    return `<td title="${esc(r && r.note)}">${r ? modPill(r.state) : "—"}</td>`;
                  })
                  .join("")}</tr>`,
            )
            .join("")}</tbody></table></div>`,
          { cls: "xl:col-span-5", id: "h-ag" },
        )}
      </div></div>`;
  }

  // -- logs
  function viewLogs(D) {
    const f = S.f.logs,
      qq = f.q.toLowerCase();
    const cnt = (lv) => D.logs.filter((l) => l.level === lv).length;
    const rows = D.logs.filter(
      (l) =>
        (f.level === "all" || l.level === f.level) && (!qq || l.raw.toLowerCase().includes(qq)),
    );
    const bar = toolbar(`${search("q-logs", f.q, "filter log lines")}
      <div class="flex flex-wrap gap-1.5" role="group" aria-label="Level">${chip("logs.level", "all", f.level, "all", D.logs.length)}${chip("logs.level", "info", f.level, "info", cnt("info"))}${chip("logs.level", "warn", f.level, "warn", cnt("warn"))}${chip("logs.level", "error", f.level, "error", cnt("error"))}</div>
      <span class="ml-auto text-2xs text-ink-subtle">${rows.length} lines · newest first</span>`);
    const body = !D.logs.length
      ? emptyNote("no logs yet")
      : !rows.length
        ? noMatch("lines")
        : `<ol class="font-mono text-xs" aria-label="log lines">${rows
            .map(
              (
                l,
              ) => `<li class="grid grid-cols-[2.5rem_minmax(0,1fr)] md:grid-cols-[2.5rem_9.5rem_3.5rem_11rem_minmax(0,1fr)] gap-x-3 gap-y-0.5 items-baseline px-3 py-1.5 border-b border-line/40 hover:bg-surface-2/60 ${l.level === "error" ? "bg-delta/[0.06]" : ""}">
        <span class="text-ink-subtle text-right text-2xs">${l.i + 1}</span>
        <span class="md:contents flex flex-wrap items-baseline gap-2"><span class="text-ink-subtle text-2xs" title="UTC">${esc(l.ts)}</span><span>${levelPill(l.level)}</span><span class="text-ink-muted truncate">${esc(l.source)}/${esc(l.name)}</span></span>
        <span class="col-start-2 md:col-start-auto break-words ${l.level === "error" ? "text-delta-fg" : l.level === "warn" ? "text-warn-fg" : "text-ink"}">${esc(l.msg)}</span></li>`,
            )
            .join("")}</ol>`;
    return `<div class="flex flex-col gap-3">${bar}${panel("logs", body, { sub: "timestamps UTC, as written", id: "h-lg" })}</div>`;
  }

  function viewLoading() {
    return `<div class="flex flex-col gap-3" aria-busy="true" aria-label="loading">
      <div class="grid grid-cols-2 sm:grid-cols-4 2xl:grid-cols-8 gap-2 md:gap-3">${Array.from({ length: 8 }, () => '<div class="glass p-3 flex flex-col gap-2"><div class="skeleton h-2.5 w-16"></div><div class="skeleton h-6 w-24"></div><div class="skeleton h-2 w-20"></div></div>').join("")}</div>
      <div class="grid grid-cols-1 xl:grid-cols-12 gap-3">${panel('<span class="skeleton inline-block h-3 w-28 align-middle"></span>', `<div class="p-3"><div class="skeleton h-40 w-full"></div></div>`, { cls: "xl:col-span-8" })}${panel('<span class="skeleton inline-block h-3 w-16 align-middle"></span>', skeletonRows(5, 3), { cls: "xl:col-span-4" })}${panel('<span class="skeleton inline-block h-3 w-24 align-middle"></span>', skeletonRows(6), { cls: "xl:col-span-12" })}</div>
      <p class="text-2xs text-ink-subtle">connecting — waiting for the first snapshot</p></div>`;
  }

  // ---------------------------------------------------------------- shell
  const statusInfo = () => {
    if (S.preview === "loading") return { cls: "text-accent-fg", label: "connecting", pulse: true };
    if (S.source === "live")
      return (
        {
          live: { cls: "text-success-fg", label: "live" },
          connecting: { cls: "text-accent-fg", label: "connecting", pulse: true },
          reconnecting: { cls: "text-warn-fg", label: "reconnecting", pulse: true },
        }[S.conn] || { cls: "text-ink-muted", label: S.conn }
      );
    return { cls: "text-accent-fg", label: "sample · offline" };
  };

  function renderShell() {
    const st = statusInfo();
    const navHtml = (mobile) =>
      ROUTES.map((r, i) => {
        const cur = S.route === r.id;
        return mobile
          ? `<a href="#/${r.id}" class="nav-item focus-ring flex-1 flex-col justify-center gap-1 h-14 px-1 text-2xs" ${cur ? 'aria-current="page"' : ""}>${icon(r.id)}<span class="truncate max-w-full">${r.title}</span></a>`
          : `<a href="#/${r.id}" class="nav-item focus-ring max-lg:justify-center" ${cur ? 'aria-current="page"' : ""} title="${r.title} (${i + 1})" aria-label="${r.title}">${icon(r.id)}<span class="hidden lg:inline">${r.title}</span><kbd class="hidden lg:inline ml-auto text-2xs text-ink-subtle font-mono">${i + 1}</kbd></a>`;
      }).join("");
    $$("[data-icon]").forEach((e) => {
      if (!e.innerHTML) e.innerHTML = icon(e.dataset.icon);
    });
    $("#side-nav").innerHTML = navHtml(false);
    $("#tab-nav").innerHTML =
      `<div class="glass flex items-stretch gap-0.5 p-1">${navHtml(true)}</div>`;
    const dot = `<span class="dot ${st.pulse ? "animate-pulse" : ""} ${st.cls}"></span>`;
    $("#side-status").innerHTML =
      `${dot}<span class="hidden lg:inline truncate">${st.label}</span>`;
    $("#side-status").title = st.label;
    $("#top-status").innerHTML =
      S.source === "live"
        ? `<span class="pill-muted ${st.cls}" role="status">${dot}${st.label}</span>`
        : "";
    $("#source-badge").innerHTML =
      S.source === "sample" && S.preview !== "loading"
        ? '<span class="pill-info" title="Not from a running rtok — sample data shaped like the /ws snapshot">sample<span class="hidden sm:inline">&nbsp;data</span></span>'
        : S.source === "live"
          ? `<span class="pill-muted hidden md:inline-flex ${st.cls}">${dot}${st.label}</span>`
          : "";
    const dark = document.documentElement.classList.contains("dark");
    $$("[data-theme-label]").forEach((e) => {
      e.textContent = dark ? "Light mode" : "Dark mode";
    });
    const r = ROUTES.find((x) => x.id === S.route);
    $("#page-title").textContent = r.title;
    $("#crumb").textContent = `rtok / ${r.sub}`;
    document.title = `${r.title} · rtok`;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = dark ? "#06101A" : "#F4F8FB";
    const err = S.snap && S.snap.error;
    $("#banners").innerHTML = err
      ? `<div class="glass mt-2 flex items-center gap-2 px-3 py-2 border-delta/60" role="alert"><span class="pill-fail">error</span><span class="text-xs text-ink min-w-0 break-words">${esc(err)}</span><button type="button" class="btn btn-ghost ml-auto shrink-0" data-action="dismiss-error">dismiss</button></div>`
      : "";
  }

  function render() {
    const a = document.activeElement;
    const keep =
      a && a.dataset && a.dataset.filter
        ? { f: a.dataset.filter, s: a.selectionStart, e: a.selectionEnd }
        : null;
    renderShell();
    const main = $("#main");
    if (S.preview === "loading" || !S.snap) {
      main.innerHTML = viewLoading();
      return;
    }
    const D = derive(S.snap);
    main.innerHTML = {
      overview: viewOverview,
      plugins: viewPlugins,
      calls: viewCalls,
      sessions: viewSessions,
      doctor: viewDoctor,
      logs: viewLogs,
    }[S.route](D);
    if (keep) {
      const el = main.querySelector(`[data-filter="${keep.f}"]`);
      if (el) {
        el.focus();
        try {
          el.setSelectionRange(keep.s, keep.e);
        } catch {
          /* search inputs */
        }
      }
    }
  }

  function toast(msg, tone = "info") {
    const t = document.createElement("div");
    t.className = `glass px-3 py-2 text-xs shadow-e3 max-w-xs ${tone === "warn" ? "border-warn/60" : ""}`;
    t.textContent = msg;
    $("#toasts").appendChild(t);
    setTimeout(() => t.remove(), 3200);
  }

  // ---------------------------------------------------------------- routing
  function parseHash() {
    const h = location.hash.replace(/^#\/?/, "");
    const [path, qs] = h.split("?");
    const params = new URLSearchParams(qs || "");
    return { route: ROUTES.some((r) => r.id === path) ? path : "overview", params };
  }
  function onRoute() {
    const { route, params } = parseHash();
    S.route = route;
    const st = params.get("state");
    if (st && st !== S.preview) setPreview(st, true);
    const id = params.get("id");
    if (id && S.snap) {
      if (route === "plugins") {
        const i = (S.snap.plugins || []).findIndex((p) => p.id === id);
        if (i >= 0) S.sel.plugin = i;
      }
      if (route === "sessions") {
        const i = (S.snap.sessions || []).findIndex((s) => s.id === id);
        if (i >= 0) S.sel.session = i;
      }
    }
    render();
    if (changed && !first) {
      window.scrollTo(0, 0);
      $("#page-title").focus({ preventScroll: true });
    }
  }

  function setPreview(p, silent) {
    S.preview = ["live", "loading", "empty", "error"].includes(p) ? p : "live";
    if (S.source !== "live") {
      if (S.preview === "empty") S.snap = emptySnapshot();
      else if (S.preview === "error") {
        S.snap = sampleSnapshot();
        S.snap.doctor = null;
        S.snap.error =
          "store will not open: database is locked (~/.rtok/rtok.db) — showing the last frame";
      } else S.snap = sampleSnapshot();
    }
    if (!silent) render();
    renderSettings();
  }

  // ---------------------------------------------------------------- live data (/ws)
  // Same protocol as crates/rtok-webui/src/lib.rs: snapshot frames, {"type":"message"},
  // {"type":"expand"}; sends {"set":{key,value}} and {"expand":id}.
  let attempt = 0,
    everOpen = false;
  function connect() {
    if (
      !/^https?:$/.test(location.protocol) ||
      new URLSearchParams(location.search).has("sample")
    ) {
      S.conn = "offline";
      return;
    }
    let ws;
    try {
      ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);
    } catch {
      S.conn = "offline";
      return;
    }
    S.ws = ws;
    ws.onopen = () => {
      everOpen = true;
      attempt = 0;
      S.conn = "live";
      renderShell();
    };
    ws.onmessage = (ev) => {
      let v;
      try {
        v = JSON.parse(ev.data);
      } catch {
        return;
      }
      if (v.type === "message") {
        toast(v.text || "error", "warn");
        return;
      }
      if (v.type === "expand") {
        S.expand.text = v.text || "";
        render();
        return;
      }
      S.source = "live";
      S.snap = v;
      S.preview = "live";
      render();
    };
    ws.onclose = () => {
      S.ws = null;
      if (!everOpen) {
        S.conn = "offline";
        renderShell();
        return;
      } // no rtok here: stay on sample data
      S.conn = "reconnecting";
      renderShell();
      setTimeout(connect, Math.min(30000, 1000 * (1 << Math.min(attempt++, 4))));
    };
  }
  const send = (obj) => {
    if (S.ws && S.ws.readyState === 1) {
      S.ws.send(JSON.stringify(obj));
      return true;
    }
    return false;
  };

  // ---------------------------------------------------------------- settings
  function renderSettings() {
    const dark = document.documentElement.classList.contains("dark");
    const motion = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const set = (k, v) => {
      const b = $(`[data-setting="${k}"]`);
      if (b) b.setAttribute("aria-checked", String(v));
    };
    set("dark", dark);
    set("orb", window.rtokOrb ? window.rtokOrb.enabled : false);
    set("opaque", document.documentElement.classList.contains("opaque"));
    const orbBtn = $('[data-setting="orb"]');
    if (orbBtn) orbBtn.disabled = motion;
    $("#orb-note").textContent = motion
      ? "Off: your system asks for reduced motion"
      : document.documentElement.dataset.orb === "fallback" &&
          window.rtokOrb &&
          window.rtokOrb.enabled
        ? "WebGL unavailable — static gradient"
        : "WebGL, ~30 fps, pauses in background tabs";
    $("#state-chips").innerHTML = ["live", "loading", "empty", "error"]
      .map(
        (p) =>
          `<button type="button" class="chip focus-ring" data-preview="${p}" aria-pressed="${S.preview === p}" ${S.source === "live" && p !== "live" ? "disabled" : ""}>${p === "live" ? "data" : p}</button>`,
      )
      .join("");
  }
  function toggleTheme() {
    const dark = !document.documentElement.classList.contains("dark");
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("rtok-theme", dark ? "dark" : "light");
    window.rtokOrb?.refresh();
    render();
    renderSettings();
  }

  // ---------------------------------------------------------------- events
  document.addEventListener("click", (e) => {
    const t = e.target.closest(
      "[data-action],[data-select],[data-toggle],[data-chip],[data-expand],[data-setting],[data-preview]",
    );
    if (!t) return;
    const a = t.dataset;
    if (a.action === "theme") return toggleTheme();
    if (a.action === "settings") {
      renderSettings();
      return $("#settings").showModal();
    }
    if (a.action === "help") return $("#help").showModal();
    if (a.action === "dismiss-error") {
      S.snap.error = undefined;
      return render();
    }
    if (a.action === "more-calls") {
      S.mobileCalls += 20;
      return render();
    }
    if (a.action === "live-only") {
      S.f.sessions.live = !S.f.sessions.live;
      return render();
    }
    if (a.action === "clear-filters") {
      const f = S.f[S.route];
      Object.keys(f).forEach((k) => {
        f[k] = typeof f[k] === "boolean" ? false : k === "q" ? "" : "all";
      });
      return render();
    }
    if (a.toggle) {
      e.stopPropagation();
      const p = S.snap.plugins.find((x) => x.id === a.toggle);
      if (!p) return;
      const next = !p.enabled;
      if (send({ set: { key: `plugins.${p.id}.enabled`, value: next } }))
        toast(`sent plugins.${p.id}.enabled = ${next}`);
      else {
        p.enabled = next;
        toast(`sample data: ${p.id} ${next ? "enabled" : "disabled"} locally (nothing sent)`);
      }
      return render();
    }
    if (a.select) {
      const i = +a.i;
      S.sel[a.select] = i;
      render();
      if (innerWidth < 1024) {
        const d = $("#detail");
        d?.scrollIntoView({
          behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
          block: "start",
        });
      }
      return;
    }
    if (a.chip) {
      const [pg, key] = a.chip.split(".");
      S.f[pg][key] = a.val;
      return render();
    }
    if (a.expand) {
      S.expand = { ref: a.expand, text: "", filter: "" };
      if (!send({ expand: a.expand }))
        S.expand.text = Array.from(
          { length: 24 },
          (_, i) =>
            `sample archived output ${a.expand} · line ${i + 1}${i % 5 === 0 ? " warning: unused variable" : ""}`,
        ).join("\n");
      return render();
    }
    if (a.setting === "dark") return toggleTheme();
    if (a.setting === "orb") {
      window.rtokOrb?.set(!window.rtokOrb.enabled);
      return renderSettings();
    }
    if (a.setting === "opaque") {
      const on = !document.documentElement.classList.contains("opaque");
      document.documentElement.classList.toggle("opaque", on);
      localStorage.setItem("rtok-opaque", on ? "on" : "off");
      return renderSettings();
    }
    if (a.preview) return setPreview(a.preview);
  });
  document.addEventListener("input", (e) => {
    const f = e.target.dataset && e.target.dataset.filter;
    if (!f) return;
    if (f === "q-expand") {
      S.expand.filter = e.target.value;
      return render();
    }
    const pg = f.replace("q-", "");
    if (S.f[pg]) {
      S.f[pg].q = e.target.value;
      render();
    }
  });
  document.addEventListener("keydown", (e) => {
    const typing =
      /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName) ||
      document.querySelector("dialog[open]");
    if (typing) {
      if (e.key === "Escape" && document.activeElement.dataset.filter)
        document.activeElement.blur();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    if (/^[1-6]$/.test(e.key)) {
      location.hash = "#/" + ROUTES[+e.key - 1].id;
      e.preventDefault();
    } else if (e.key === "/") {
      const s = $("#main input[type=search]");
      if (s) {
        s.focus();
        e.preventDefault();
      }
    } else if (e.key === "?") $("#help").showModal();
    else if (
      (e.key === "ArrowDown" || e.key === "ArrowUp") &&
      ["plugins", "calls", "sessions"].includes(S.route)
    ) {
      const k = { plugins: "plugin", calls: "call", sessions: "session" }[S.route];
      const list = S.snap[S.route] || [];
      if (!list.length) return;
      S.sel[k] = Math.max(
        0,
        Math.min(list.length - 1, S.sel[k] + (e.key === "ArrowDown" ? 1 : -1)),
      );
      e.preventDefault();
      render();
      const row = $(`#main tr[aria-selected="true"]`);
      row?.scrollIntoView({ block: "nearest" });
    }
  });
  addEventListener("hashchange", () => onRoute(false));
  matchMedia("(prefers-reduced-motion: reduce)").addEventListener("change", renderSettings);

  // ---------------------------------------------------------------- boot
  const initialState = parseHash().params.get("state");
  S.snap = sampleSnapshot();
  setPreview(initialState || "live", true);
  onRoute(true);
  renderSettings();
  connect();
})();
