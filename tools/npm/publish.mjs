#!/usr/bin/env node
// Publishes the tarballs tools/npm/build.mjs left in target/npm/dist: every platform package
// first, `rtok` last, so no one installs a launcher whose binary is not on the registry yet.
// Manual only; no workflow runs this. Refuses to start when a tarball is missing or any
// version differs from Cargo.toml.
//
//   node tools/npm/publish.mjs --dry-run       # npm publish --dry-run for each tarball
//   node tools/npm/publish.mjs [--tag next]    # the real thing; needs `npm login` beforehand

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import {
    MAIN_PKG,
    PLATFORMS,
    cargoMetadata,
    outDirs,
    releasePackages,
    run,
    tarballName,
} from "./common.mjs";

const { values: opts } = parseArgs({
    options: {
        "dry-run": { type: "boolean", default: false },
        tag: { type: "string" },
        help: { type: "boolean", short: "h", default: false },
    },
});

if (opts.help) {
    console.log("usage: tools/npm/publish.mjs [--dry-run] [--tag <dist-tag>]");
    process.exit(0);
}

function manifestOf(tarball) {
    const json = execFileSync("tar", ["-xzOf", tarball, "package/package.json"], {
        encoding: "utf8",
    });
    return JSON.parse(json);
}

/** Every reason not to publish; empty when the tarballs are complete and in sync. */
function problems(version, dist) {
    const found = [];
    for (const pkg of releasePackages()) {
        const tarball = path.join(dist, tarballName(pkg, version));
        if (!fs.existsSync(tarball)) {
            found.push(`missing ${tarball}`);
            continue;
        }
        const manifest = manifestOf(tarball);
        if (manifest.name !== pkg || manifest.version !== version) {
            found.push(
                `${tarball} is ${manifest.name}@${manifest.version}, expected ${pkg}@${version}`,
            );
        }
        if (pkg === MAIN_PKG) {
            const deps = manifest.optionalDependencies ?? {};
            for (const { pkg: platform } of PLATFORMS) {
                if (deps[platform] !== version) {
                    found.push(
                        `rtok's optionalDependencies has ${platform}@${deps[platform]}, expected ${version}`,
                    );
                }
            }
        }
    }
    return found;
}

function main() {
    const { version, targetDir } = cargoMetadata();
    const { dist } = outDirs(targetDir);
    const found = problems(version, dist);
    if (found.length > 0) {
        console.error(`refusing to publish rtok ${version}:`);
        for (const p of found) {
            console.error(`  - ${p}`);
        }
        console.error("build every platform first: node tools/npm/build.mjs --release v" + version);
        process.exit(1);
    }
    if (!opts["dry-run"]) {
        try {
            execFileSync("npm", ["whoami"], { stdio: "ignore" });
        } catch {
            console.error("refusing to publish: not logged in to npm (run `npm login` first)");
            process.exit(1);
        }
    }
    for (const pkg of releasePackages()) {
        const args = ["publish", path.join(dist, tarballName(pkg, version)), "--access", "public"];
        if (opts.tag) {
            args.push("--tag", opts.tag);
        }
        if (opts["dry-run"]) {
            args.push("--dry-run");
        }
        console.log(`\n== npm ${args.join(" ")}`);
        run("npm", args);
    }
    console.log(`\n${opts["dry-run"] ? "dry run of" : "published"} rtok ${version}`);
}

main();
