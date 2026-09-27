// A fake `rtok` for the host plugins' Node unit tests (T47.3), on every OS: the running `node`
// binary is linked (or copied) as `rtok[.exe]` once per process, and each `fakeRtok(body)` points
// `NODE_OPTIONS=--require` at a script that plays `rtok`. `--version` is answered by node
// itself (exit 0); any other call runs `body` with `args` (argv after the binary) and `input`
// (stdin) in scope, then exits 0. `fakeRtok(null)` leaves `rtok` off PATH.
import { copyFileSync, linkSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";

const exe = process.platform === "win32" ? "rtok.exe" : "rtok";
let binDir: string | undefined;

function bin(): string {
  if (binDir) return binDir;
  const dir = mkdtempSync(join(tmpdir(), "rtok-fake-bin-"));
  const dest = join(dir, exe);
  try {
    if (process.platform === "win32") linkSync(process.execPath, dest);
    else symlinkSync(process.execPath, dest);
  } catch {
    copyFileSync(process.execPath, dest);
  }
  return (binDir = dir);
}

export function fakeRtok(body: string | null): void {
  if (body === null) {
    process.env.PATH = mkdtempSync(join(tmpdir(), "rtok-fake-none-"));
    delete process.env.NODE_OPTIONS;
    return;
  }
  const script = join(mkdtempSync(join(tmpdir(), "rtok-fake-")), "rtok.cjs");
  writeFileSync(
    script,
    `const args = [require("path").basename(process.argv[1]), ...process.argv.slice(2)];\n` +
      `const input = require("fs").readFileSync(0, "utf8");\n${body}\nprocess.exit(0);\n`,
  );
  process.env.PATH = [bin(), process.env.PATH ?? ""].join(delimiter);
  process.env.NODE_OPTIONS = `--require "${script.replace(/\\/g, "/")}"`;
}

/** How long a wedged fake waits before it answers "too late" anyway: only a fallback, so a
 * broken kill fails the test instead of leaking a child for long. The plugins' 5 s spawn
 * timeout must kill the fake first, so this stays well clear of it: a fake answering just
 * past 5 s (say 5.1 s) could beat a kill delayed by a loaded box and turn a pass into a
 * flake. Timing bounds in the tests are kept below it, so passing them proves the kill
 * (not this fallback) ended the wait. */
export const HANG_FALLBACK_MS = 10_000;

/** The stdin marker that makes `fakeHangingRtok({ onlyMarked: true })` wedge `subcommand`. */
export const wedgeMarker = (subcommand: string): string => `rtok-wedge:${subcommand}`;

// One script per distinct source, so fakes set up with the same options leave the same
// `NODE_OPTIONS` behind: concurrent cases sharing one fake then cannot race on the env.
const hangScripts = new Map<string, string>();

/** A fake `rtok` that wedges (T214): the plugins' 5 s spawn timeout must kill it and fail
 * open. By default every call wedges. With `onlyMarked`, a call wedges only when its stdin
 * carries `wedgeMarker(<its subcommand>)` and otherwise echoes stdin back at once, so one
 * case can time out exactly one spawn of a handler (the `hook` or the `filter` call).
 * The wait is a timer, not a blocking `Atomics.wait`, so the event loop stays free and the
 * explicit SIGTERM handler runs: it re-raises the signal with the default action, so the
 * child dies at the timeout/abort kill and the parent sees a signal death as with a real
 * wedged process. `--require` scripts run before the main module, so `Module.runMain` is
 * stubbed to keep node from loading the subcommand (`hook`, `filter`) as a script. */
export function fakeHangingRtok({ onlyMarked = false, ms = HANG_FALLBACK_MS } = {}): void {
  const source = `const args = [require("path").basename(process.argv[1]), ...process.argv.slice(2)];
const input = require("fs").readFileSync(0, "utf8");
if (${onlyMarked} && !input.includes(${JSON.stringify(wedgeMarker(""))} + args[0])) {
  process.stdout.write(input);
  process.exit(0);
}
require("module").runMain = () => {};
process.once("SIGTERM", () => process.kill(process.pid, "SIGTERM"));
setTimeout(() => {
  process.stdout.write("too late");
  process.exit(0);
}, ${ms});
`;
  let script = hangScripts.get(source);
  if (!script) {
    script = join(mkdtempSync(join(tmpdir(), "rtok-fake-hang-")), "rtok.cjs");
    writeFileSync(script, source);
    hangScripts.set(source, script);
  }
  process.env.PATH = [bin(), process.env.PATH ?? ""].join(delimiter);
  process.env.NODE_OPTIONS = `--require "${script.replace(/\\/g, "/")}"`;
}

/** A fake `rtok` that exits `code` at once without reading stdin, so a large
 * write from the plugin hits a closed pipe (EPIPE): the plugin must fail open
 * instead of crashing on the unhandled stream `error`. */
export function fakeExitingRtok(code = 3): void {
  const script = join(mkdtempSync(join(tmpdir(), "rtok-fake-exit-")), "rtok.cjs");
  writeFileSync(script, `process.exit(${code});\n`);
  process.env.PATH = [bin(), process.env.PATH ?? ""].join(delimiter);
  process.env.NODE_OPTIONS = `--require "${script.replace(/\\/g, "/")}"`;
}
