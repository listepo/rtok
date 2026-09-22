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
    // Windows mise leaves `@vitest/mocker` with a broken `vite` peer symlink; put `npm:vite`'s
    // `node_modules` on `NODE_PATH` so `import "vite"` resolves (Mac/Linux already ship vite
    // inside the vitest install tree).
    if let Some(vite_nm) = mise_npm_node_modules("vite") {
        let sep = if cfg!(windows) { ";" } else { ":" };
        let node_path = match std::env::var("NODE_PATH") {
            Ok(existing) if !existing.is_empty() => {
                format!("{}{}{}", vite_nm.display(), sep, existing)
            }
            _ => vite_nm.display().to_string(),
        };
        cmd.env("NODE_PATH", node_path);
    }
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("{exe}: {e} (run under `mise exec --`)"));
    assert!(status.success(), "{file}");
}

/// Root `node_modules` of a mise `npm:<pkg>` install, if `mise where` finds one.
fn mise_npm_node_modules(pkg: &str) -> Option<std::path::PathBuf> {
    let output = Command::new("mise")
        .args(["where", &format!("npm:{pkg}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = std::path::PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    let nm = root.join("node_modules");
    if nm.is_dir() {
        Some(nm)
    } else if root.is_dir() {
        Some(root)
    } else {
        None
    }
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
