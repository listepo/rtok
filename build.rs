//! Embeds the git sha into `rtok --version` (plan T10.4): `rtok 0.1.0 (1a2b3c4d5)`.
//! Without a `.git` (crates.io tarball, dist source archive) the sha reads `unknown`.

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
}
