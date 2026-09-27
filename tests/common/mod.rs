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
///
/// Linux only: the plugins are plain TypeScript with no OS-specific paths, so one OS covers
/// them; on macOS and Windows this is a no-op.
pub fn vitest(file: &str, envs: &[(&str, &Path)]) {
    if !cfg!(target_os = "linux") {
        eprintln!("skip vitest {file}: host plugin tests run on Linux only");
        return;
    }
    // npm installs a `.cmd` shim on Windows, which `Command` only finds by its full name.
    let exe = if cfg!(windows) {
        "vitest.cmd"
    } else {
        "vitest"
    };
    let mut cmd = Command::new(exe);
    // `--bail=1`: stop at the first failing test; the panic below names the file either way.
    cmd.args(["run", "--bail=1", file])
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

/// T266: a fake `~/.ketch/bin/rtok` that prints `ketch <event>` for `rtok hook <event> …`.
pub const KETCH_ECHO: &str = "#!/bin/sh\nprintf 'ketch %s' \"$2\"\n";

/// T266: runs a plugin's hook command the way its host does, with a throwaway `HOME` and an
/// empty `PATH` — so no real `rtok`, on PATH or in `~/.ketch/bin`, is reachable. The empty PATH
/// dir lives under `HOME`; both go on drop.
#[cfg(unix)]
pub struct HookShell {
    home: std::path::PathBuf,
    empty_path: std::path::PathBuf,
}

#[cfg(unix)]
impl HookShell {
    pub fn new(name: &str) -> Self {
        let home = agents::tmp(name);
        let empty_path = home.join("empty-path");
        std::fs::create_dir_all(&empty_path).unwrap();
        Self { home, empty_path }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// `/bin/sh -c <command>`; see [`HookShell::sh`].
    pub fn run(&self, command: &str) -> (bool, String) {
        self.sh(&["-c", command])
    }

    /// `/bin/sh <args>` with `{}` on stdin and stderr dropped: (exited 0, stdout).
    pub fn sh(&self, args: &[&str]) -> (bool, String) {
        use std::io::Write;
        use std::process::Stdio;
        let mut child = Command::new("/bin/sh")
            .args(args)
            .env("HOME", &self.home)
            .env("PATH", &self.empty_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // A fail-open hook may exit before reading stdin: a broken pipe is fine (T251).
        drop(child.stdin.take().unwrap().write_all(b"{}"));
        let out = child.wait_with_output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
        )
    }

    /// Writes `script` as an executable `~/.ketch/bin/rtok`.
    pub fn install_fake_ketch_rtok(&self, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        let fake = self.home.join(".ketch/bin/rtok");
        std::fs::create_dir_all(fake.parent().unwrap()).unwrap();
        std::fs::write(&fake, script).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[cfg(unix)]
impl Drop for HookShell {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
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
