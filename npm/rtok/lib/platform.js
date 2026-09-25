"use strict";
// One table of the npm platform packages. The launcher (bin/rtok), the postinstall step
// (install.js) and tools/npm-build.mjs all read it, so a new target is added here only.
// `target` is the Rust triple dist builds (dist-workspace.toml); `os`, `cpu` and `libc` are
// the npm fields that let npm skip the packages that do not match the host.

const path = require("node:path");

const PLATFORMS = [
  { pkg: "rtok-darwin-arm64", os: "darwin", cpu: "arm64", target: "aarch64-apple-darwin" },
  {
    pkg: "rtok-linux-x64-gnu",
    os: "linux",
    cpu: "x64",
    libc: "glibc",
    target: "x86_64-unknown-linux-gnu",
  },
  { pkg: "rtok-win32-x64-msvc", os: "win32", cpu: "x64", target: "x86_64-pc-windows-msvc" },
];

/** The executable name of `name` on `os` (`rtok.exe` on Windows). */
function exeName(os, name = "rtok") {
  return os === "win32" ? `${name}.exe` : name;
}

/** "glibc" or "musl" on Linux, undefined elsewhere (the same test esbuild and biome use). */
function hostLibc() {
  if (process.platform !== "linux") {
    return undefined;
  }
  const header = process.report?.getReport()?.header;
  return header?.glibcVersionRuntime ? "glibc" : "musl";
}

/** The PLATFORMS row for the running host, or undefined when rtok ships no binary for it. */
function hostPlatform() {
  const libc = hostLibc();
  return PLATFORMS.find(
    (p) => p.os === process.platform && p.cpu === process.arch && (!p.libc || p.libc === libc),
  );
}

function describeHost() {
  const libc = hostLibc();
  return `${process.platform}-${process.arch}${libc ? ` (${libc})` : ""}`;
}

/** Absolute path of the native rtok binary for this host; throws a readable Error if absent. */
function nativeBinary() {
  const platform = hostPlatform();
  if (!platform) {
    const supported = PLATFORMS.map((p) => p.pkg).join(", ");
    throw new Error(
      `no prebuilt rtok binary for ${describeHost()}. Supported: ${supported}. ` +
        "Install from source instead: https://github.com/listepo/rtok#install",
    );
  }
  let dir;
  try {
    dir = path.dirname(require.resolve(`${platform.pkg}/package.json`));
  } catch {
    throw new Error(
      `the platform package ${platform.pkg} is not installed. It is an optional dependency of ` +
        "rtok; reinstall without --no-optional / --omit=optional, e.g. `npm i -g rtok`.",
    );
  }
  return path.join(dir, "bin", exeName(platform.os));
}

module.exports = { PLATFORMS, exeName, hostPlatform, nativeBinary };
