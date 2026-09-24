// T48.1: pi's own loader finds the extension through the directory `rtok agents install pi --yes`
// links into `<agent dir>/extensions/rtok`. pi reads that directory's `package.json`
// `pi.extensions` (then `index.ts`), so no `index.ts` is needed. `RTOK_PI_AGENT_DIR` points at a
// dir install prepared; without it the test links `plugins/pi` itself. Skipped when pi is missing.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const PI = "@earendil-works/pi-coding-agent";

// Windows never runs a bare `pi`: npm shims a package's bin as `pi.cmd` (cmd.exe), `pi.ps1`
// (PowerShell) or `pi.exe` (a native launcher) depending on how it was installed, and none of
// those resolve through a plain `pi` lookup — only the bare name covers macOS/Linux.
const BIN_NAMES = ["pi", "pi.cmd", "pi.exe", "pi.ps1"];

/** Root of the pi package behind `pi` (or a PATHEXT shim) on PATH, or null. */
function piPackage(): string | null {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    for (const name of BIN_NAMES) {
      let bin: string;
      try {
        bin = fs.realpathSync(path.join(dir, name));
      } catch {
        continue;
      }
      for (let d = path.dirname(bin); d !== path.dirname(d); d = path.dirname(d)) {
        try {
          if (JSON.parse(fs.readFileSync(path.join(d, "package.json"), "utf8")).name === PI)
            return d;
        } catch {}
      }
    }
  }
  return null;
}

const pkg = piPackage();
if (!pkg) {
  console.warn(
    `skipped pi loader test (T48.1): no ${BIN_NAMES.join("/")} on PATH resolves to ${PI}`,
  );
}

test.skipIf(!pkg)("pi loads the linked rtok directory once", async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rtok-pi-load-"));
  let agentDir = process.env.RTOK_PI_AGENT_DIR;
  if (!agentDir) {
    agentDir = path.join(tmp, "agent");
    fs.mkdirSync(path.join(agentDir, "extensions"), { recursive: true });
    const plugin = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
    fs.symlinkSync(plugin, path.join(agentDir, "extensions", "rtok"), "junction");
  }
  const { discoverAndLoadExtensions } = await import(
    pathToFileURL(path.join(pkg!, "dist/index.js")).href
  );
  const { extensions, errors } = await discoverAndLoadExtensions([], tmp, agentDir);
  expect(errors).toEqual([]);
  expect(extensions, extensions.map((e: any) => e.path).join(", ")).toHaveLength(1);
  const [ext] = extensions;
  expect(ext.path).toMatch(/extensions[\\/]rtok[\\/]extensions[\\/]rtok\.ts$/);
  expect(ext.handlers.has("tool_call"), "bash rewrite handler").toBe(true);
  expect(ext.handlers.has("tool_result"), "bash filter handler").toBe(true);
  expect(ext.handlers.has("context"), "archive live-zone handler (T70.2)").toBe(true);
  fs.rmSync(tmp, { recursive: true, force: true });
});
