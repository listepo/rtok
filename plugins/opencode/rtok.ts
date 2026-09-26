import { spawnSync } from "node:child_process";

export type AfterInput = {
  tool: string;
  sessionID?: string;
  args?: { command?: string; filePath?: string; path?: string; name?: string };
};

export type AfterOutput = {
  title?: string;
  output: string;
  metadata?: unknown;
};

export type FilterFn = (cmd: string, stdin: string) => string;
export type HookFn = (event: string, stdin: string) => string;
export type GuardFn = (
  tool: string,
  args: unknown,
  session: string,
) => { allow: boolean; reason?: string };

const KETCH_HINT =
  "rtok is not installed; tool output is passed through unfiltered.\n" +
  "Install with ketch:  ketch install listepo/rtok";
let hinted = false;

type Spawn = { missing: boolean; failed: boolean; stdout: string };

/** Fail open: on spawn/error, return the original stdin. A missing `rtok` says so once (D21).
 * T214: `timeout` bounds a wedged `rtok` — the kill surfaces as `r.error`
 * (ETIMEDOUT), so it lands on the `failed` path and the caller keeps the original. */
function spawnRtok(args: string[], stdin = ""): Spawn {
  const r = spawnSync("rtok", args, { input: stdin, encoding: "utf8", timeout: 5000 });
  const missing = Boolean(r.error && (r.error as NodeJS.ErrnoException).code === "ENOENT");
  if (missing && !hinted) {
    hinted = true;
    console.error(KETCH_HINT);
  }
  return {
    missing,
    failed: Boolean(r.error) || r.status !== 0,
    stdout: r.stdout ?? "",
  };
}

export function filterStdin(cmd: string, stdin: string): string {
  // Kilo 7 may hand a non-string through a mis-bound hook; coerce so load/run never throws.
  const c = typeof cmd === "string" ? cmd : String(cmd ?? "");
  const args = ["filter", "--stdin", "--cmd", c];
  if (/^skill(\s|$)/i.test(c.trim())) args.push("--archive");
  const r = spawnRtok(args, stdin);
  if (r.failed) return stdin;
  return r.stdout;
}

function additionalContext(stdout: string): string {
  try {
    const v = JSON.parse(stdout);
    return typeof v?.hookSpecificOutput?.additionalContext === "string"
      ? v.hookSpecificOutput.additionalContext
      : "";
  } catch {
    return "";
  }
}

/** `rtok hook <event> --host opencode`; empty on fail-open. */
export function hookStdin(event: string, stdin: string): string {
  const r = spawnRtok(["hook", event, "--host", "opencode"], stdin);
  if (r.failed) return "";
  return additionalContext(r.stdout);
}

function claudeTool(name: string): string {
  const l = String(name ?? "").toLowerCase();
  if (l === "bash" || l.includes("shell") || l.includes("terminal")) return "Bash";
  if (l.startsWith("read") || l.startsWith("view")) return "Read";
  if (l === "edit") return "Edit";
  if (l === "write") return "Write";
  return String(name ?? "");
}

function claudeArgs(args: unknown): unknown {
  if (!args || typeof args !== "object") return args ?? {};
  const o = args as Record<string, unknown>;
  if (o.filePath !== undefined && o.file_path === undefined && o.path === undefined) {
    const { filePath, ...rest } = o;
    return { ...rest, file_path: filePath };
  }
  return args;
}

/** `rtok guard check`; missing/failed/unparsable/no-reason → allow. */
export function guardCheck(
  tool: string,
  args: unknown,
  session: string,
): { allow: boolean; reason?: string } {
  const r = spawnRtok([
    "guard",
    "check",
    "--tool",
    tool,
    "--json",
    JSON.stringify(claudeArgs(args)),
    "--session",
    session,
    "--host",
    "opencode",
  ]);
  if (r.failed) return { allow: true };
  try {
    const v = JSON.parse(r.stdout);
    if (v && v.allow === false && typeof v.reason === "string" && v.reason) {
      return { allow: false, reason: v.reason };
    }
  } catch {
    // fail open
  }
  return { allow: true };
}

function remember(tool: string, args: unknown, session: string, output: string) {
  const name = claudeTool(tool);
  if (name !== "Bash" && name !== "Read" && name !== "Edit" && name !== "Write") return;
  spawnRtok(
    ["hook", "PostToolUse", "--host", "opencode"],
    JSON.stringify({
      hook_event_name: "PostToolUse",
      session_id: session,
      tool_name: name,
      tool_input: claudeArgs(args),
      tool_response: { stdout: output },
    }),
  );
}

function hookPayload(event: string, sessionID: string, extra: object = {}): string {
  return JSON.stringify({ hook_event_name: event, session_id: sessionID, ...extra });
}

/** Single-quote for `rtok run -- '…'` (same shape as the Claude PreToolUse rewrite). */
function shellQuote(cmd: string): string {
  return `'${cmd.replace(/'/g, `'"'"'`)}'`;
}

/** OpenCode + Kilo plugin: bash → `rtok run`, guard, skill filter, compaction (T70.6 / T97).
 * Kilo 7 (and current OpenCode) require `export default { id, server }` — a bare function
 * default makes the host's plugin.config / dispose path throw (`w.config` undefined). */
export function createPlugin(
  run: FilterFn = filterStdin,
  hook: HookFn = hookStdin,
  check: GuardFn = guardCheck,
) {
  let restore = "";
  return async () => ({
    "tool.execute.before": async (
      input: { tool?: string; sessionID?: string },
      output: { args?: Record<string, unknown> },
    ) => {
      const tool = String(input?.tool ?? "");
      const v = check(tool, output?.args ?? {}, String(input?.sessionID ?? ""));
      if (!v.allow && v.reason) throw new Error(v.reason);
      // Rewrite bash to `rtok run` before the host executes it (T97 live check; matches
      // Claude/pi). Skip when already wrapped so a second before-hook cannot nest.
      if (
        tool.toLowerCase() === "bash" &&
        output?.args &&
        typeof output.args.command === "string"
      ) {
        const cmd = output.args.command;
        if (!/^\s*rtok(\s|$)/.test(cmd)) {
          output.args.command = `rtok run -- ${shellQuote(cmd)}`;
        }
      }
    },
    "tool.execute.after": async (input: AfterInput, output: AfterOutput) => {
      if (run === filterStdin) {
        remember(String(input.tool), input.args, String(input.sessionID ?? ""), output.output);
      }
      const tool = String(input.tool).toLowerCase();
      if (tool === "skill") {
        output.output = run(`skill ${String(input.args?.name ?? "")}`, output.output);
        return;
      }
      if (tool !== "bash") return;
      // `rtok run` already filtered; do not filter again.
      if (/^\s*rtok\s+run\b/.test(String(input.args?.command ?? ""))) return;
      output.output = run(String(input.args?.command ?? ""), output.output);
    },
    "experimental.session.compacting": async (
      input: { sessionID?: string },
      output: { context: string[]; prompt?: string },
    ) => {
      const sid = String(input?.sessionID ?? "");
      hook("PreCompact", hookPayload("PreCompact", sid, { trigger: "auto" }));
      const text = hook("SessionStart", hookPayload("SessionStart", sid, { source: "compact" }));
      if (text) {
        output.context.push(text);
        restore = sid;
      }
    },
    "experimental.chat.system.transform": async (
      input: { sessionID?: string },
      output: { system: string[] },
    ) => {
      if (!restore || (input?.sessionID && String(input.sessionID) !== restore)) return;
      const text = hook("PostCompact", hookPayload("PostCompact", restore));
      restore = "";
      if (text) output.system.push(text);
    },
  });
}

export default { id: "rtok", server: createPlugin() };
