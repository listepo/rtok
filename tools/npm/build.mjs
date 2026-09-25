#!/usr/bin/env node
// Builds the npm tarballs into target/npm/dist: `rtok` (the launcher, esbuild layout) plus one
// package per platform with the native binary, rtok-hook and the plugins/ and skills/ trees
// the binary resolves beside itself. Versions come from Cargo.toml.
//
//   node tools/npm/build.mjs                   # cargo build for the host, pack host + rtok
//   node tools/npm/build.mjs --target <triple> # cross build (the toolchain must be set up)
//   node tools/npm/build.mjs --no-build        # reuse target/<profile> as it is
//   node tools/npm/build.mjs --release v0.7.0  # every platform from the dist Release archives
//
// A publish takes `--release`: those archives are the signed binaries dist ships.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import {
  LICENSE_FILES,
  MAIN_DIR,
  MAIN_PKG,
  PLATFORMS,
  ROOT,
  cargoMetadata,
  exeName,
  outDirs,
  run,
  tarballName,
} from "./common.mjs";

const REPO = "listepo/rtok";
const DATA_DIRS = ["plugins", "skills"];
const HOOK_BIN = "rtok-hook";

const { values: opts } = parseArgs({
  options: {
    release: { type: "string" },
    target: { type: "string", multiple: true },
    "no-build": { type: "boolean", default: false },
    profile: { type: "string", default: "release" },
    help: { type: "boolean", short: "h", default: false },
  },
});

if (opts.help) {
  console.log(
    "usage: tools/npm/build.mjs [--release vX.Y.Z | --target <triple>... | --no-build] [--profile <name>]",
  );
  process.exit(0);
}

function hostTriple() {
  const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  return out.match(/^host: (\S+)$/m)[1];
}

function platformFor(target) {
  const platform = PLATFORMS.find((p) => p.target === target);
  if (!platform) {
    const known = PLATFORMS.map((p) => p.target).join(", ");
    throw new Error(`no npm platform package for ${target} (known: ${known})`);
  }
  return platform;
}

/** cargo's output directory for a profile (`dev` builds land in `debug`). */
function profileDir(profile) {
  return profile === "dev" ? "debug" : profile;
}

/** Sources for local builds: binaries from target/, plugins/ and skills/ from the checkout. */
function localSources(targetDir) {
  const host = hostTriple();
  const targets = opts.target ?? [host];
  return targets.map((target) => {
    const platform = platformFor(target);
    const cross = target !== host;
    if (!opts["no-build"]) {
      const args = ["build", "--locked", "--profile", opts.profile, "-p", MAIN_PKG, "--bins"];
      run("cargo", cross ? [...args, "--target", target] : args, { cwd: ROOT });
    }
    const binDir = cross
      ? path.join(targetDir, target, profileDir(opts.profile))
      : path.join(targetDir, profileDir(opts.profile));
    return { platform, binDir, dataDir: ROOT };
  });
}

/** Sources for a publish: the dist archives of the GitHub Release `tag`, every platform. */
function releaseSources(tag, version, downloads) {
  if (tag !== `v${version}`) {
    throw new Error(`--release ${tag} does not match Cargo.toml version ${version}`);
  }
  return PLATFORMS.map((platform) => {
    const archive = `rtok-${platform.target}${platform.os === "win32" ? ".zip" : ".tar.xz"}`;
    const dir = path.join(downloads, platform.target);
    fs.mkdirSync(dir, { recursive: true });
    run("gh", ["release", "download", tag, "-R", REPO, "-p", archive, "-D", dir, "--clobber"]);
    // bsdtar (macOS, Windows) reads zip too; GNU tar does not, so use unzip there.
    if (archive.endsWith(".zip") && process.platform === "linux") {
      run("unzip", ["-q", "-o", archive], { cwd: dir });
    } else {
      run("tar", ["-xf", archive], { cwd: dir });
    }
    // The unix archives hold one `rtok-<target>/` directory; the Windows zip is flat.
    const nested = path.join(dir, `rtok-${platform.target}`);
    const root = fs.existsSync(nested) ? nested : dir;
    return { platform, binDir: root, dataDir: root };
  });
}

function copyLicenses(dest) {
  for (const file of LICENSE_FILES) {
    fs.copyFileSync(path.join(ROOT, file), path.join(dest, file));
  }
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function pack(dir, dist) {
  run("npm", ["pack", "--pack-destination", dist, "--silent"], { cwd: dir });
}

function stagePlatform({ platform, binDir, dataDir }, main, stage) {
  const dir = path.join(stage, platform.pkg);
  const bin = path.join(dir, "bin");
  fs.mkdirSync(bin, { recursive: true });
  const rtok = path.join(binDir, exeName(platform.os));
  if (!fs.existsSync(rtok)) {
    throw new Error(`missing ${rtok}`);
  }
  for (const name of [MAIN_PKG, HOOK_BIN]) {
    const src = path.join(binDir, exeName(platform.os, name));
    if (fs.existsSync(src)) {
      fs.copyFileSync(src, path.join(bin, path.basename(src)));
      fs.chmodSync(path.join(bin, path.basename(src)), 0o755);
    }
  }
  for (const data of DATA_DIRS) {
    const src = path.join(dataDir, data);
    if (!fs.existsSync(src)) {
      throw new Error(`missing ${src}: the binary resolves ${data}/ beside itself`);
    }
    fs.cpSync(src, path.join(bin, data), { recursive: true });
  }
  copyLicenses(dir);
  fs.writeFileSync(
    path.join(dir, "README.md"),
    `# ${platform.pkg}\n\nThe \`${platform.target}\` binary of [rtok](https://www.npmjs.com/package/rtok). ` +
      "Install `rtok`, not this package; npm picks it for you.\n",
  );
  writeJson(path.join(dir, "package.json"), {
    name: platform.pkg,
    version: main.version,
    description: `The ${platform.target} binary of rtok; install the rtok package instead`,
    homepage: main.homepage,
    bugs: main.bugs,
    repository: main.repository,
    license: main.license,
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.libc ? { libc: [platform.libc] } : {}),
    files: ["bin/", ...LICENSE_FILES],
    preferUnplugged: true,
  });
  return dir;
}

function stageMain(version, stage) {
  const dir = path.join(stage, MAIN_PKG);
  fs.cpSync(MAIN_DIR, dir, { recursive: true });
  copyLicenses(dir);
  const file = path.join(dir, "package.json");
  const main = JSON.parse(fs.readFileSync(file, "utf8"));
  main.version = version;
  main.optionalDependencies = Object.fromEntries(PLATFORMS.map((p) => [p.pkg, version]));
  writeJson(file, main);
  return { dir, main };
}

function main() {
  const { version, targetDir } = cargoMetadata();
  const { out, stage, dist } = outDirs(targetDir);
  fs.rmSync(out, { recursive: true, force: true });
  fs.mkdirSync(stage, { recursive: true });
  fs.mkdirSync(dist, { recursive: true });

  const sources = opts.release
    ? releaseSources(opts.release, version, path.join(out, "downloads"))
    : localSources(targetDir);

  const { dir: mainDir, main: mainJson } = stageMain(version, stage);
  for (const source of sources) {
    pack(stagePlatform(source, mainJson, stage), dist);
  }
  pack(mainDir, dist);

  const built = new Set(sources.map((s) => s.platform.pkg));
  const missing = PLATFORMS.filter((p) => !built.has(p.pkg)).map((p) => p.pkg);
  console.log(`\nrtok ${version} → ${dist}`);
  for (const file of fs.readdirSync(dist).sort()) {
    console.log(`  ${file}`);
  }
  if (missing.length > 0) {
    console.log(`not built (a publish needs them; use --release): ${missing.join(", ")}`);
  }
  console.log(
    `install locally: npm i ${path.join(dist, tarballName(MAIN_PKG, version))} <platform tgz>`,
  );
}

main();
