//! Helpers shared by the integration tests (`mod common;` in each file that uses one).
// Every test binary that includes this module compiles all of it; none uses every helper.
#![allow(dead_code)]

pub mod agents;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// T111: run one TS host plugin test file under vitest (the mise tool; `vitest.config.mjs` at the
/// repo root). `envs` are set for the run. Panics with the file name when a test fails.
pub fn vitest(file: &str, envs: &[(&str, &Path)]) {
    // npm installs a `.cmd` shim on Windows, which `Command` only finds by its full name.
    let exe = if cfg!(windows) {
        "vitest.cmd"
    } else {
        "vitest"
    };
    let status = Command::new(exe)
        .args(["run", file])
        .envs(envs.iter().copied())
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap_or_else(|e| panic!("{exe}: {e} (run under `mise exec --`)"));
    assert!(status.success(), "{file}");
}

/// Nearest-rank p95 of sorted samples: the smallest sample at or above 95 % of them. The old
/// `samples[n * 95 / 100]` was one rank high — at n = 20 it was the maximum, so one slow spawn on
/// a loaded CI runner failed a latency gate that 19 fast ones had passed.
pub fn p95(sorted: &[Duration]) -> Duration {
    sorted[(sorted.len() * 95).div_ceil(100) - 1]
}
