// T48.1: pi's own loader finds the extension through the directory `rtok agents install pi --yes`
// links into `<agent dir>/extensions/rtok`. pi reads that directory's `package.json`
// `pi.extensions` (then `index.ts`), so no `index.ts` is needed. `RTOK_PI_AGENT_DIR` points at a
// dir install prepared; without it the test links `plugins/pi` itself. Skips when pi is missing.
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const PI = "@earendil-works/pi-coding-agent";

/** Root of the pi package behind `pi` on PATH, or null. */
function piPackage(): string | null {
  for (const dir of (process.env.PATH ?? "").split(path.delimiter)) {
    let bin: string;
    try {
      bin = fs.realpathSync(path.join(dir, "pi"));
    } catch {
      continue;
    }
    for (let d = path.dirname(bin); d !== path.dirname(d); d = path.dirname(d)) {
      try {
        if (JSON.parse(fs.readFileSync(path.join(d, "package.json"), "utf8")).name === PI) return d;
      } catch {}
    }
  }
  return null;
}

const pkg = piPackage();

test("pi loads the linked rtok directory once", { skip: !pkg && "pi not installed" }, async () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "rtok-pi-load-"));
  let agentDir = process.env.RTOK_PI_AGENT_DIR;
  if (!agentDir) {
    agentDir = path.join(tmp, "agent");
    fs.mkdirSync(path.join(agentDir, "extensions"), { recursive: true });
    const plugin = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
    fs.symlinkSync(plugin, path.join(agentDir, "extensions", "rtok"), "junction");
  }
  const { discoverAndLoadExtensions } = await import(pathToFileURL(path.join(pkg!, "dist/index.js")).href);
  const { extensions, errors } = await discoverAndLoadExtensions([], tmp, agentDir);
  assert.deepEqual(errors, []);
  assert.equal(extensions.length, 1, extensions.map((e: any) => e.path).join(", "));
  const [ext] = extensions;
  assert.match(ext.path, /extensions[\\/]rtok[\\/]extensions[\\/]rtok\.ts$/);
  assert.ok(ext.handlers.has("tool_call"), "bash rewrite handler");
  assert.ok(ext.handlers.has("tool_result"), "bash filter handler");
  fs.rmSync(tmp, { recursive: true, force: true });
});
