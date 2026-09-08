// rtok pi extension (T10.6, D21): the single bash call path, no MCP.
//
// pi philosophy is no MCP: this extension does NOT register tools. It only
// rewrites `bash` calls to `rtok run -- …` (archived, filtered, measured) and
// compresses `bash` results through `rtok filter`, with an `expand <id>`
// trailer that recovers the full output (lossless by default, D4).
// Missing `rtok` fails open and names the ketch install (D21).
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
    // execFile without a callback `input` option: feed stdin manually.
    if (input !== undefined) {
      child.stdin.write(input);
      child.stdin.end();
    }
  });
}

export default function (pi) {
  // One call path: bash → `rtok run -- <command>`. Not a duplicate of any
  // MCP read/search: pi has no MCP, and the hook never touches other tools.
  pi.on("tool_call", async (event) => {
    if (event.toolName !== "bash") return;
    const command = event.input?.command;
    if (typeof command !== "string" || command.startsWith("rtok run -- ")) return;
    const r = await rtok(["run", "--", command]);
    if (r.missing) {
      pi.appendEntry?.("system", KETCH_HINT);
      return;
    }
    // `rtok run` already executed, archived and filtered the command: run it
    // through the filter path by replacing the command, so the result below
    // still compresses oversized output the same way.
    event.input.command = `rtok run -- ${command}`;
  });

  // Bash results: `rtok filter` compresses oversized output; the trailer
  // carries `expand <id>` for the full text. Small output passes through.
  pi.on("tool_result", async (event) => {
    if (event.toolName !== "bash") return;
    const text = (event.content ?? [])
      .map((c) => (typeof c?.text === "string" ? c.text : ""))
      .join("\n");
    if (!text) return;
    const r = await rtok(["filter", "--stdin"], text, undefined);
    if (r.missing || !r.stdout) return;
    const out = r.stdout.trimEnd();
    if (out && out !== text.trimEnd()) {
      return { content: [{ type: "text", text: out }] };
    }
  });

  // Optional proxy (T11.5 pattern): route pi through `rtok proxy`.
  // pi.registerProvider("anthropic", {
  //   baseUrl: "http://127.0.0.1:8790/v1",
  // });
}
