// rtok pi extension (T10.6, D21): the single bash call path, no MCP.
//
// pi philosophy is no MCP: this extension does NOT register tools. It
// rewrites `bash` calls to `rtok run -- …` (archived, filtered, measured),
// compresses `bash` results through `rtok filter`, and shrinks the pi
// `context` message array through `rtok archive rewrite` (T70.2) — the same
// archive live zone the proxy runs, without a proxy. Every shortened payload
// carries an `expand <id>` trailer (lossless by default, D4). Missing `rtok`
// fails open and names the ketch install (D21).
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

function rtok(args, input, signal) {
  return new Promise((resolve) => {
    const child = execFile("rtok", args, { signal }, (error, stdout, stderr) => {
      if (error && error.code === "ENOENT") {
        resolve({ missing: true });
        return;
      }
      // Fail open (D1): any plugin error keeps the unmodified input/output.
      resolve({ stdout: String(stdout ?? ""), stderr: String(stderr ?? "") });
    });
    // execFile without a callback `input` option: feed stdin, then close it
    // so a child that reads stdin (the test fake, `guard check`) cannot hang.
    if (input !== undefined) {
      child.stdin.write(input);
    }
    child.stdin.end();
  });
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
      if (event.toolName === "bash") pi.appendEntry?.("system", KETCH_HINT);
    } else {
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
    event.input.command = `rtok run -- ${quoted}`;
  });

  // Bash results: `rtok filter` compresses oversized output; the trailer
  // carries `expand <id>` for the full text. Small output passes through.
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
    if (event.toolName !== "bash") return;
    if (!text) return;
    const r = await rtok(["filter", "--stdin"], text, undefined);
    if (r.missing || !r.stdout) return;
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
    const r = await rtok(["archive", "rewrite", "--stdin"], input, undefined);
    if (!r.missing && r.stdout && r.stdout !== input) {
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
      const text = additionalContext(c.stdout);
      if (text) {
        messages = [
          ...messages,
          { role: "user", content: [{ type: "text", text }] },
        ];
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
