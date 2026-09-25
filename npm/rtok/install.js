"use strict";
// postinstall: on Unix, put the native binary where npm linked `rtok` (bin/rtok), next to the
// plugins/ and skills/ it resolves beside itself, so a hook that runs `rtok` from PATH does
// not pay for a Node start (the hook budget is 10 ms). Fails open: on any error the Node
// launcher stays in place and still works. Windows keeps the launcher, because npm's .cmd
// shim runs bin/rtok through node.

const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");
const { exeName, hostPlatform, nativeBinary } = require("./lib/platform.js");
const { version } = require("./package.json");

function sameVersion(binary) {
  // `rtok 0.7.0 (6f4a360e6)`: the second word is the version.
  const out = execFileSync(binary, ["--version"], { encoding: "utf8" });
  return out.trim().split(/\s+/)[1] === version;
}

function linkOrCopy(src, dest) {
  const tmp = `${dest}.tmp-${process.pid}`;
  fs.rmSync(tmp, { force: true });
  try {
    fs.linkSync(src, tmp);
  } catch {
    fs.copyFileSync(src, tmp);
  }
  fs.chmodSync(tmp, 0o755);
  fs.renameSync(tmp, dest);
}

function main() {
  if (process.platform === "win32" || !hostPlatform()) {
    return;
  }
  const native = nativeBinary();
  if (!sameVersion(native)) {
    throw new Error(`${native} does not report version ${version}`);
  }
  const srcDir = path.dirname(native);
  const binDir = path.join(__dirname, "bin");
  const launcher = path.join(binDir, exeName(process.platform));
  // Everything the binary ships beside itself first (plugins/, skills/, rtok-hook), the
  // binary last, so an interrupted install never leaves a binary without its plugins.
  for (const entry of fs.readdirSync(srcDir)) {
    if (entry === path.basename(native)) {
      continue;
    }
    const src = path.join(srcDir, entry);
    const dest = path.join(binDir, entry);
    if (fs.statSync(src).isDirectory()) {
      fs.cpSync(src, dest, { recursive: true, force: true });
    } else {
      linkOrCopy(src, dest);
    }
  }
  linkOrCopy(native, launcher);
}

try {
  main();
} catch (error) {
  console.warn(`rtok: kept the Node launcher (${error.message})`);
}
