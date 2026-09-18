// rtok pi extension (T10.6, T70.1, D21): one spawn helper, no MCP.
//
// pi philosophy is no MCP: this extension does NOT register tools. It
// rewrites `bash` calls to `rtok run -- …` (archived, filtered, measured),
// compresses `bash` / `read` / `grep` / `find` / `ls` results through
// `rtok filter`, and shrinks the pi `context` message array through
// `rtok archive rewrite` (T70.2) — the same call path as `rtok proxy`
// (T11.5 pattern, `http://127.0.0.1:8790/v1`).

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
    const child = execFile("rtok", args, { signal }, (error, stdout, stderr) => {
      if (error && error.code === "ENOENT") {
        resolve({ missing: true });
        return;
      }
      // Fail open (D1): any plugin error keeps the unmodified input/output.
      resolve({ stdout: String(stdout ?? ""), stderr: String(stderr ?? "") });
    });
    // execFile without a callback `input` option: feed stdin manually.
    if (input !== undefined) {
      child.stdin.write(input);
      child.stdin.end();
    }
  });
}

function hintMissing(pi) {
  if (pi._rtokHinted) return;
  pi._rtokHinted = true;
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
  // One call path: bash → `rtok run -- <command>`. Not a duplicate of any
  // MCP read/search: pi has no MCP, and the hook never touches other tools.
  pi.on("tool_call", async (event) => {
    if (event.toolName !== "bash") return;
    const command = event.input?.command;
    if (typeof command !== "string" || command.startsWith("rtok run -- ")) return;
    // Probe install only: `rtok run` would execute the command before bash does.
    const r = await rtok(["--version"]);
    if (r.missing) {
      hintMissing(pi);
      return;
    }
    const quoted = `'${command.replace(/'/g, `'"'"'`)}'`;
    event.input.command = `rtok run -- ${quoted}`;
  });

  // Bash results: `rtok filter` compresses oversized output. File/search
  // tools pass `--cmd "<tool> <path-or-pattern>"` so the cmd family matches.
  // The trailer carries `expand <id>` for the full text. Small output passes through.
  pi.on("tool_result", async (event) => {
    const args = filterArgs(event);
    if (!args) return;
    const text = (event.content ?? [])
      .map((c) => (typeof c?.text === "string" ? c.text : ""))
      .join("\n");
    if (!text) return;
    const r = await rtok(args, text, undefined);
    if (r.missing) {
      hintMissing(pi);
      return;
    }
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
    const input = JSON.stringify(event.messages);
    const r = await rtok(["archive", "rewrite", "--stdin"], input, undefined);
    if (r.missing || !r.stdout || r.stdout === input) return;
    try {
      return { messages: JSON.parse(r.stdout) };
    } catch {
      return; // fail open: unparseable output keeps the untouched array
    }
  });

  // Optional proxy (T11.5 pattern): route pi through `rtok proxy`.
  // pi.registerProvider("anthropic", {
  //   baseUrl: "http://127.0.0.1:8790/v1",
  // });
}
