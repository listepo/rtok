// rtok pi extension (T10.6, T70.1, D21): one spawn helper, no MCP.
//
// pi philosophy is no MCP: bash still goes through `rtok run` / `rtok filter`.
// File/search results go through `rtok filter --cmd` (T70.1). When `[setup.pi]
// tools = true`, `pi.registerTool` exposes the measured MCP set as thin
// `rtok mcp --call` wrappers (T70.3) — one call path, not a second read/search.
// Off by default so those descriptions do not ride every request.
// Every shortened payload carries an `expand <id>` trailer (D4). Missing `rtok`
// fails open and names the ketch install (D21).
//
// oh my pi (`omp`, T92) loads this same extension through the legacy
// `pi.extensions` manifest key. It is told apart by `pi.pi` (omp injects its
// SDK there; pi's API has no such member). omp has native MCP, so the tools
// come from `rtok mcp` there and `registerTool` stays off (D21).
//
// Optional proxy: uncomment the `registerProvider` block to route pi's
// provider through `rtok proxy` (T11.5 pattern, `http://127.0.0.1:8790/v1`).

import { execFile } from "node:child_process";

const KETCH_HINT = [
  "rtok is not installed.",
  "",
  "Install with ketch:",
  "  ketch install listepo/rtok",
  "",
  "If ketch is not installed:",
  "  curl -fsSL https://raw.githubusercontent.com/listepo/ketch/main/install.sh | bash",
  "  ketch install listepo/rtok",
].join("\n");

const FILE_TOOLS = new Set(["read", "grep", "find", "ls"]);

function rtok(args, input, signal) {
  return new Promise((resolve) => {
    // T214: `timeout` bounds a wedged `rtok` (DB lock, broken pipe) so the
    // host session never freezes; `signal` lets the host abort cut it short.
    const child = execFile("rtok", args, { timeout: 5000, signal }, (error, stdout, stderr) => {
      if (error && error.code === "ENOENT") {
        resolve({ missing: true });
        return;
      }
      // Fail open (D1): any plugin error (non-zero exit, timeout kill, abort)
      // resolves `failed`, and every handler below keeps the original content
      // on it. The keep-original handling lives here, not in T195.
      if (error) {
        resolve({ failed: true, stdout: String(stdout ?? ""), stderr: String(stderr ?? "") });
        return;
      }
      resolve({ stdout: String(stdout ?? ""), stderr: String(stderr ?? "") });
    });
    // execFile without a callback `input` option: feed stdin, then close it
    // so a child that reads stdin (the test fake, `guard check`) cannot hang.
    // A child that dies first (timeout kill, abort, missing binary) makes the
    // write fail with EPIPE; unhandled, that `error` would crash the host
    // instead of failing open. The callback above already reports the failure.
    child.stdin.on("error", () => {});
    if (input !== undefined) {
      child.stdin.write(input);
    }
    child.stdin.end();
  });
}

function hintMissing(pi) {
  if (pi._rtokHinted) return;
  pi._rtokHinted = true;
  // T48.2 / I-36: the hint must reach the model through `pi.sendMessage`
  // (LLM context), not `pi.appendEntry` (TUI-only, invisible to the model).
  // Older pi builds without `sendMessage` still get the TUI entry.
  if (typeof pi.sendMessage === "function") {
    pi.sendMessage({ customType: "rtok-missing", content: KETCH_HINT, display: true });
    return;
  }
  pi.appendEntry?.("system", KETCH_HINT);
}

/** `rtok filter` argv for this result, or null when the tool is left alone. */
function filterArgs(event) {
  if (event.toolName === "bash") return ["filter", "--stdin"];
  if (!FILE_TOOLS.has(event.toolName)) return null;
  const arg = event.input?.path ?? event.input?.pattern;
  const hint = typeof arg === "string" && arg ? `${event.toolName} ${arg}` : event.toolName;
  return ["filter", "--stdin", "--cmd", hint];
}

export default function (pi) {
  let restore = false;
  let compactSession = "";
  // One call path: bash → `rtok run -- <command>`. Not a duplicate of any
  // MCP read/search: pi has no MCP, and the hook never touches other tools.
  pi.on("tool_call", async (event, ctx) => {
    const session = String(ctx?.sessionId ?? ctx?.sessionID ?? "");
    const g = await rtok(
      [
        "guard",
        "check",
        "--tool",
        String(event.toolName ?? ""),
        "--json",
        JSON.stringify(event.input ?? {}),
        "--session",
        session,
        "--host",
        "pi",
      ],
      undefined,
      event?.signal,
    );
    if (g.missing) {
      // Route through the same once-per-session guard as `tool_result`'s
      // hint (T195): a raw `appendEntry` here bypassed `hintMissing` and
      // appended one TUI entry per bash call instead of once.
      if (event.toolName === "bash") hintMissing(pi);
    } else if (!g.failed) {
      try {
        const v = JSON.parse(g.stdout);
        if (v && v.allow === false && typeof v.reason === "string" && v.reason) {
          return { block: true, reason: v.reason };
        }
      } catch {
        // fail open: unparsable output allows the call
      }
    }
    if (event.toolName !== "bash") return;
    const command = event.input?.command;
    if (typeof command !== "string" || command.startsWith("rtok run -- ")) return;
    if (g.missing) return;
    const quoted = `'${command.replace(/'/g, `'"'"'`)}'`;
    // pi documents mutating `event.input`; omp documents returning `{ input }`.
    event.input.command = `rtok run -- ${quoted}`;
    return { input: event.input };
  });

  // Bash results: `rtok filter` compresses oversized output. File/search
  // tools pass `--cmd "<tool> <path-or-pattern>"` so the cmd family matches.
  // The trailer carries `expand <id>` for the full text. Small output passes through.
  pi.on("tool_result", async (event, ctx) => {
    const text = (event.content ?? [])
      .map((c) => (typeof c?.text === "string" ? c.text : ""))
      .join("\n");
    const session = String(ctx?.sessionId ?? ctx?.sessionID ?? "");
    const tool = claudeTool(event.toolName);
    if (text && (tool === "Bash" || tool === "Read" || tool === "Edit" || tool === "Write")) {
      await rtok(
        ["hook", "PostToolUse", "--host", "pi"],
        JSON.stringify({
          hook_event_name: "PostToolUse",
          session_id: session,
          tool_name: tool,
          tool_input: event.input ?? {},
          tool_response: { stdout: text },
        }),
      );
    }
    const args = filterArgs(event);
    if (!args || !text) return;
    const r = await rtok(args, text, event?.signal);
    if (r.missing) {
      hintMissing(pi);
      return;
    }
    if (r.failed) return;
    if (!r.stdout) return;
    const out = r.stdout.trimEnd();
    if (out && out !== text.trimEnd()) {
      return { content: [{ type: "text", text: out }] };
    }
  });

  // The archive live zone without a proxy (T70.2). pi fires `context` before
  // every LLM call with the full message array (a deep copy) and sends the
  // returned `{ messages }` — so the rewrite must be idempotent. It is:
  // `rtok archive rewrite` persists each pointer decision and echoes the
  // input bytes back when nothing is eligible, so "no change" is a cheap
  // string compare and pi keeps the exact same array object.
  pi.on("context", async (event) => {
    if (!Array.isArray(event.messages)) return;
    let messages = event.messages;
    const input = JSON.stringify(event.messages);
    const r = await rtok(["archive", "rewrite", "--stdin"], input, event?.signal);
    if (!r.missing && !r.failed && r.stdout && r.stdout !== input) {
      try {
        messages = JSON.parse(r.stdout);
      } catch {
        // fail open: unparseable output keeps the untouched array
      }
    }
    if (restore) {
      restore = false;
      const c = await rtok(
        ["hook", "PostCompact", "--host", "pi"],
        JSON.stringify({
          hook_event_name: "PostCompact",
          session_id: compactSession,
        }),
      );
      const text = c.failed ? "" : additionalContext(c.stdout);
      if (text) {
        messages = [...messages, { role: "user", content: [{ type: "text", text }] }];
      }
    }
    if (messages === event.messages) return;
    return { messages };
  });

  // Compaction (T70.6): save via PreCompact, restore on the next `context`
  // call. session_before_compact can only cancel or *replace* the host
  // summary — rtok never returns `compaction.summary` (that would drop
  // what pi knows). T58.2 owns hook hosts; this is the plugin path.
  pi.on("session_before_compact", async (event, ctx) => {
    compactSession = String(ctx?.sessionId ?? ctx?.sessionID ?? "");
    const trigger = event?.reason === "manual" ? "manual" : "auto";
    await rtok(
      ["hook", "PreCompact", "--host", "pi"],
      JSON.stringify({
        hook_event_name: "PreCompact",
        session_id: compactSession,
        trigger,
      }),
      event?.signal,
    );
  });

  pi.on("session_compact", async (_event, ctx) => {
    compactSession = String(ctx?.sessionId ?? ctx?.sessionID ?? compactSession);
    restore = true;
  });

  pi.on("session_start", async () => {
    await registerPiTools(pi);
  });

  // Optional proxy (T11.5 pattern): route pi through `rtok proxy`.
  // pi.registerProvider("anthropic", {
  //   baseUrl: "http://127.0.0.1:8790/v1",
  // });
}

function claudeTool(name) {
  const l = String(name ?? "").toLowerCase();
  if (l === "bash" || l.includes("shell") || l.includes("terminal")) return "Bash";
  if (l.startsWith("read") || l.startsWith("view")) return "Read";
  if (l === "edit") return "Edit";
  if (l === "write") return "Write";
  return String(name ?? "");
}

const PI_TOOLS = [
  {
    name: "read",
    label: "Read",
    description: "Read a file; mode full|lines|map|signatures; range a-b for full|lines.",
    parameters: {
      type: "object",
      properties: { path: { type: "string" }, mode: { type: "string" }, range: { type: "string" } },
      required: ["path"],
    },
  },
  {
    name: "search",
    label: "Search",
    description: "Regex search files; path:line: snippet, max hits.",
    parameters: {
      type: "object",
      properties: {
        pattern: { type: "string" },
        path: { type: "string" },
        max: { type: "integer" },
      },
      required: ["pattern"],
    },
  },
  {
    name: "tree",
    label: "Tree",
    description: "Compact directory listing with sizes; depth cap.",
    parameters: {
      type: "object",
      properties: { path: { type: "string" }, depth: { type: "integer" } },
    },
  },
  {
    name: "symbol",
    label: "Symbol",
    description:
      "Definitions of a symbol with their source: path:line kind, then the body. Optional path substring and kind narrow the match.",
    parameters: {
      type: "object",
      properties: { name: { type: "string" }, path: { type: "string" }, kind: { type: "string" } },
      required: ["name"],
    },
  },
  {
    name: "callers",
    label: "Callers",
    description:
      "Which definitions reference a symbol: path, calling definition, count. Optional path substring keeps one subtree.",
    parameters: {
      type: "object",
      properties: { name: { type: "string" }, path: { type: "string" } },
      required: ["name"],
    },
  },
  {
    name: "expand",
    label: "Expand",
    description:
      "Return archived payload by id; optional lines a-b, regex grep (hits as N:line), context N.",
    parameters: {
      type: "object",
      properties: {
        id: { type: "string" },
        lines: { type: "string" },
        grep: { type: "string" },
        context: { type: "integer" },
      },
      required: ["id"],
    },
  },
  {
    name: "mem_search",
    label: "Mem search",
    description: "Search notes by FTS5; ids, titles, snippets.",
    parameters: {
      type: "object",
      properties: { query: { type: "string" }, limit: { type: "integer" } },
      required: ["query"],
    },
  },
  {
    name: "mem_get",
    label: "Mem get",
    description: "Return one note body by id.",
    parameters: { type: "object", properties: { id: { type: "integer" } }, required: ["id"] },
  },
];

async function registerPiTools(pi) {
  if (typeof pi.registerTool !== "function") return;
  // omp: native MCP already serves these tools — a second path would break D21.
  if (typeof pi.pi === "object" && pi.pi !== null) return;
  const r = await rtok(["config", "get", "setup.pi.tools"]);
  if (r.missing || r.stdout.trim() !== "true") return;
  for (const t of PI_TOOLS) {
    pi.registerTool({
      name: t.name,
      label: t.label,
      description: t.description,
      parameters: t.parameters,
      execute: async (_id, params, signal) => {
        const out = await rtok(
          ["mcp", "--call", t.name, "--json", JSON.stringify(params ?? {})],
          undefined,
          signal,
        );
        if (out.missing) {
          return { content: [{ type: "text", text: KETCH_HINT }] };
        }
        if (out.failed) {
          return {
            content: [
              { type: "text", text: `rtok ${t.name} failed; retry or continue without it` },
            ],
          };
        }
        return { content: [{ type: "text", text: String(out.stdout ?? "") }] };
      },
    });
  }
}

function additionalContext(stdout) {
  try {
    const v = JSON.parse(stdout);
    return typeof v?.hookSpecificOutput?.additionalContext === "string"
      ? v.hookSpecificOutput.additionalContext
      : "";
  } catch {
    return "";
  }
}
