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

/** A fake `rtok` that wedges for `ms` before answering (T214): the plugins'
 * 5 s spawn timeout must kill it and fail open. Synchronous `Atomics.wait`
 * so no async plumbing is needed in the `--require` script. */
export function fakeHangingRtok(ms = 30_000): void {
  fakeRtok(
    `Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ${ms});\n` +
      `process.stdout.write("too late");`,
  );
}
