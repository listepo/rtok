import { spawnSync } from "node:child_process";

export type AfterInput = {
  tool: string;
  args?: { command?: string };
};

export type AfterOutput = {
  title?: string;
  output: string;
  metadata?: unknown;
};

export type FilterFn = (cmd: string, stdin: string) => string;
export type HookFn = (event: string, stdin: string) => string;

const KETCH_HINT =
  "rtok is not installed; bash output is passed through unfiltered.\n" +
  "Install with ketch:  ketch install listepo/rtok";
let hinted = false;

type Spawn = { missing: boolean; failed: boolean; stdout: string };

/** Fail open: on spawn/error, return the original stdin. A missing `rtok` says so once (D21). */
function spawnRtok(args: string[], stdin = ""): Spawn {
  const r = spawnSync("rtok", args, { input: stdin, encoding: "utf8" });
  const missing = Boolean(
    r.error && (r.error as NodeJS.ErrnoException).code === "ENOENT",
  );
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
  const r = spawnRtok(["filter", "--stdin", "--cmd", cmd], stdin);
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

function hookPayload(event: string, sessionID: string, extra: object = {}): string {
  return JSON.stringify({ hook_event_name: event, session_id: sessionID, ...extra });
}

/** OpenCode plugin: bash filter plus compaction checkpoint (T70.6). */
export function createPlugin(run: FilterFn = filterStdin, hook: HookFn = hookStdin) {
  let restore = "";
  return async () => ({
    "tool.execute.after": async (input: AfterInput, output: AfterOutput) => {
      if (String(input.tool).toLowerCase() !== "bash") return;
      output.output = run(String(input.args?.command ?? ""), output.output);
    },
    "experimental.session.compacting": async (
      input: { sessionID?: string },
      output: { context: string[]; prompt?: string },
    ) => {
      const sid = String(input?.sessionID ?? "");
      hook("PreCompact", hookPayload("PreCompact", sid, { trigger: "auto" }));
      const text = hook(
        "SessionStart",
        hookPayload("SessionStart", sid, { source: "compact" }),
      );
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

export default createPlugin();
