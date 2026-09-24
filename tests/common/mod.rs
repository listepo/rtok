//! Helpers shared by the integration tests (`mod common;` in each file that uses one).
// Every test binary that includes this module compiles all of it; none uses every helper.
#![allow(dead_code)]

pub mod agents;
#[cfg(unix)]
pub mod fake_lsp;

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
    let mut cmd = Command::new(exe);
    cmd.args(["run", file])
        .envs(envs.iter().copied())
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    // Windows mise leaves `@vitest/mocker` without its `vite` peer, and ESM ignores NODE_PATH:
    // preload a resolve hook that retries `vite` from the `npm:vite` install (T83.4).
    if let Some(vite) = mise_npm_root("vite") {
        let hook = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/node/vite-peer.mjs");
        let path = agents::slash(hook.to_string_lossy());
        let url = format!("file:///{}", path.trim_start_matches('/'));
        let opts = std::env::var("NODE_OPTIONS").unwrap_or_default();
        cmd.env("NODE_OPTIONS", format!("--import=\"{url}\" {opts}"))
            .env("RTOK_VITE_ROOT", vite);
    }
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("{exe}: {e} (run under `mise exec --`)"));
    assert!(status.success(), "{file}");
}

/// Install root of a mise `npm:<pkg>` tool, if `mise where` finds one.
fn mise_npm_root(pkg: &str) -> Option<std::path::PathBuf> {
    let output = Command::new("mise")
        .args(["where", &format!("npm:{pkg}")])
        .output()
        .ok()?;
    let root = std::path::PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    (output.status.success() && root.join("node_modules").is_dir()).then_some(root)
}

/// Nearest-rank p95 of sorted samples: the smallest sample at or above 95 % of them. The old
/// `samples[n * 95 / 100]` was one rank high — at n = 20 it was the maximum, so one slow spawn on
/// a loaded CI runner failed a latency gate that 19 fast ones had passed.
pub fn p95(sorted: &[Duration]) -> Duration {
    sorted[(sorted.len() * 95).div_ceil(100) - 1]
}

/// T75/T178: another process's writer on the store at `db` — `BEGIN IMMEDIATE` held for `hold`.
/// Returns once the lock is taken; join the handle to wait for its release.
pub fn hold_store_writer(db: &Path, hold: Duration) -> std::thread::JoinHandle<()> {
    use diesel::Connection;
    use diesel::connection::SimpleConnection;
    let (held, held_ack) = std::sync::mpsc::channel();
    let url = db.to_str().unwrap().to_string();
    let holder = std::thread::spawn(move || {
        let mut conn = diesel::sqlite::SqliteConnection::establish(&url).unwrap();
        conn.batch_execute("PRAGMA busy_timeout = 1000; PRAGMA journal_mode = WAL;")
            .unwrap();
        conn.batch_execute("BEGIN IMMEDIATE;").unwrap();
        held.send(()).unwrap();
        std::thread::sleep(hold);
        conn.batch_execute("COMMIT;").unwrap();
    });
    held_ack.recv().unwrap();
    holder
}
