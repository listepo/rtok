//! Embeds the git sha into `rtok --version` (plan T10.4): `rtok 0.1.0 (1a2b3c4d5)`.
//! Without a `.git` (crates.io tarball, dist source archive) the sha reads `unknown`.
//!
//! T310.9: also picks the directory `rtok web` embeds as its UI, the built React SPA in
//! `web/dist`. `RTOK_WEB_EMBED=require` (the release job) turns a missing SPA into a build
//! failure instead of a binary without a UI.

use std::path::Path;
use std::process::Command;

fn main() {
    let sha = Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=RTOK_GIT_SHA={sha}");
    // T93: Windows reserves 1 MiB for the main thread where Linux and macOS give 8 MiB, and
    // the debug `cli::run` frame overflowed it on every command. Reserve the same 8 MiB —
    // address space, not committed memory, so the hook path pays nothing.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        let arg = if msvc {
            "/STACK:8388608"
        } else {
            "-Wl,--stack,8388608"
        };
        println!("cargo:rustc-link-arg-bins={arg}");
    }
    // Rebuild when HEAD moves; skip the hints when there is no repository (a missing
    // path would make cargo rerun this script on every build).
    for p in [".git/HEAD", ".git/refs/heads", ".git/packed-refs"] {
        if Path::new(p).exists() {
            println!("cargo:rerun-if-changed={p}");
        }
    }
    web_bundle();
}

/// The built SPA `src/web/spa.rs` embeds.
const SPA_DIST: &str = "web/dist";

/// T310.9: `RTOK_SPA_DIR` names the directory `rust-embed` bakes into the binary: `web/dist`
/// when it holds a built SPA, otherwise a one-page placeholder that says how to build it, so a
/// contributor without npm still gets a working `cargo build` (fail open) and the binary
/// answers `/` with instructions instead of a bare 404. The release job sets
/// `RTOK_WEB_EMBED=require` to turn a missing SPA into a build failure.
fn web_bundle() {
    println!("cargo:rerun-if-env-changed=RTOK_WEB_EMBED");
    let dist = Path::new(SPA_DIST);
    if dist.join("index.html").is_file() {
        // Only an existing path: naming a missing one makes cargo rerun this script, and
        // rebuild the crate, on every build (measured). `just spa-build` touches build.rs, so a
        // binary built before the SPA picks it up; `just web` reads the directory at run time.
        println!("cargo:rerun-if-changed={SPA_DIST}");
        let abs = dist.canonicalize().expect("web/dist is readable");
        println!("cargo:rustc-env=RTOK_SPA_DIR={}", abs.display());
        return;
    }
    if std::env::var("RTOK_WEB_EMBED").as_deref() == Ok("require") {
        panic!(
            "RTOK_WEB_EMBED=require but {SPA_DIST}/index.html is missing; run `npm ci && npm run build` in web/ first"
        );
    }
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("spa-placeholder");
    std::fs::create_dir_all(&out).expect("create placeholder dir");
    std::fs::write(out.join("index.html"), PLACEHOLDER).expect("write placeholder");
    println!("cargo:rustc-env=RTOK_SPA_DIR={}", out.display());
    println!("cargo:rustc-env=RTOK_SPA_PLACEHOLDER=1");
}

const PLACEHOLDER: &str = "<!doctype html>
<html lang=\"en\"><head><meta charset=\"utf-8\"><title>rtok</title>
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head>
<body style=\"font:16px/1.5 system-ui,sans-serif;max-width:40rem;margin:4rem auto;padding:0 1rem\">
<h1>rtok web: the UI is not in this build</h1>
<p>This binary was built without the React app (<code>web/dist</code> did not exist), so only the
API and <code>/ws</code> are up.</p>
<p>Build it with <code>just web</code> (runs <code>npm ci &amp;&amp; npm run build</code> in
<code>web/</code> and serves the result), or build it yourself and point
<code>RTOK_WEB_DIST</code> at the <code>dist/</code> directory. A release build embeds it.</p>
</body></html>
";
