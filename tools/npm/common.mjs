// Shared by tools/npm/build.mjs and tools/npm/publish.mjs: where the tarballs go, which ones a
// release needs, and the version they must carry (the `rtok` package in Cargo.toml).
// The npm name is `rtok-cli` (`rtok` on npm is not ours to use); the command is `rtok`.

import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = fileURLToPath(new URL("../..", import.meta.url));
export const MAIN_DIR = path.join(ROOT, "npm", "rtok-cli");
export const MAIN_PKG = "rtok-cli";
// The Cargo package whose binaries and version the npm packages carry.
export const CARGO_PKG = "rtok";
// Shipped in every package: the three licenses the README offers.
export const LICENSE_FILES = ["LICENSE", "LICENSE-ROYALTY-FREE.md", "PRICING.md"];

const require = createRequire(import.meta.url);
export const { PLATFORMS, exeName } = require(path.join(MAIN_DIR, "lib", "platform.js"));

/** Run a command with inherited stdio; throws on a non-zero exit. */
export function run(cmd, args, options = {}) {
    execFileSync(cmd, args, { stdio: "inherit", ...options });
}

/** Version of the `rtok` package and cargo's target directory, from `cargo metadata`. */
export function cargoMetadata() {
    const out = execFileSync(
        "cargo",
        ["metadata", "--no-deps", "--format-version", "1", "--locked"],
        { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    const meta = JSON.parse(out);
    const rtok = meta.packages.find((p) => p.name === CARGO_PKG);
    if (!rtok) {
        throw new Error("no `rtok` package in cargo metadata");
    }
    return { version: rtok.version, targetDir: meta.target_directory };
}

/** target/npm and its parts: `stage/` (package trees) and `dist/` (the tarballs). */
export function outDirs(targetDir) {
    const out = path.join(targetDir, "npm");
    return { out, stage: path.join(out, "stage"), dist: path.join(out, "dist") };
}

/** The file `npm pack` writes for an unscoped package. */
export function tarballName(pkg, version) {
    return `${pkg}-${version}.tgz`;
}

/** Every package a release publishes, platform packages first, `rtok-cli` last. */
export function releasePackages() {
    return [...PLATFORMS.map((p) => p.pkg), MAIN_PKG];
}
