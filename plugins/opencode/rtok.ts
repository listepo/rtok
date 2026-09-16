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

const KETCH_HINT =
  "rtok is not installed; bash output is passed through unfiltered.\n" +
  "Install with ketch:  ketch install listepo/rtok";
let hinted = false;

/** Fail open: on spawn/error, return the original stdin. A missing `rtok` says so once (D21). */
export function filterStdin(cmd: string, stdin: string): string {
  const r = spawnSync("rtok", ["filter", "--stdin", "--cmd", cmd], {
    input: stdin,
    encoding: "utf8",
  });
  if (r.error && (r.error as NodeJS.ErrnoException).code === "ENOENT" && !hinted) {
    hinted = true;
    console.error(KETCH_HINT);
  }
  if (r.error || r.status !== 0 || r.stdout == null) return stdin;
  return r.stdout;
}

/** OpenCode plugin: `tool.execute.after` replaces bash output via `rtok filter`. */
export function createPlugin(run: FilterFn = filterStdin) {
  return async () => ({
    "tool.execute.after": async (input: AfterInput, output: AfterOutput) => {
      if (String(input.tool).toLowerCase() !== "bash") return;
      output.output = run(String(input.args?.command ?? ""), output.output);
    },
  });
}

export default createPlugin();
