// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! Child-process capture that never waits on a pipe's EOF: shared by `rtok run` (T235.1) and
//! the host `--version` probe (T280).

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// How long [`capture`] keeps reading once the process has exited. A descendant it left
/// running (`git fsmonitor--daemon`, `gradle --daemon`, a backgrounded server, a host CLI's
/// first-run helper) can hold the pipe's write end for good, so waiting for EOF would hang.
const DRAIN_AFTER_EXIT: Duration = Duration::from_millis(200);

/// How often [`capture`] checks a time-limited child.
const POLL: Duration = Duration::from_millis(10);

/// What [`capture`] read, and the exit code: `None` when a signal ended the process or it
/// was killed at the time limit.
pub(crate) struct Captured {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    #[cfg_attr(not(feature = "cmd"), allow(dead_code))]
    pub code: Option<i32>,
}

/// Run `cmd` with stdin closed and stdout/stderr piped. Waits for the process rather than for
/// EOF (unlike `Command::output`): after it exits, what its pipes already hold is drained for
/// up to [`DRAIN_AFTER_EXIT`], and a reader still blocked by a descendant is left behind.
/// With `limit`, a process still running after it is killed.
pub(crate) fn capture(mut cmd: Command, limit: Option<Duration>) -> std::io::Result<Captured> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let (tx, rx) = mpsc::channel();
    let out = drain(child.stdout.take(), tx.clone());
    let err = drain(child.stderr.take(), tx);
    let code = match limit {
        None => child.wait()?.code(),
        Some(limit) => wait_until(&mut child, Instant::now() + limit)?,
    };
    let deadline = Instant::now() + DRAIN_AFTER_EXIT;
    for _ in 0..2 {
        if rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_err()
        {
            break;
        }
    }
    let take =
        |b: &Mutex<Vec<u8>>| std::mem::take(&mut *b.lock().unwrap_or_else(PoisonError::into_inner));
    Ok(Captured {
        stdout: take(&out),
        stderr: take(&err),
        code,
    })
}

/// The child's exit code, or `None` after killing it at `deadline`.
fn wait_until(child: &mut Child, deadline: Instant) -> std::io::Result<Option<i32>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(POLL);
    }
}

/// Read `pipe` to EOF on its own thread into the returned buffer; signal `done` at EOF.
fn drain(pipe: Option<impl Read + Send + 'static>, done: mpsc::Sender<()>) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&buf);
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 8192];
            while let Ok(n @ 1..) = pipe.read(&mut chunk) {
                sink.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend_from_slice(&chunk[..n]);
            }
        }
        let _ = done.send(());
    });
    buf
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// T235.1: a backgrounded grandchild keeps the capture pipe open after the shell exits.
    /// `capture` returns on the shell's exit with its output and code, not at the pipe's EOF.
    #[test]
    fn capture_returns_when_the_child_exits_though_a_grandchild_holds_the_pipe() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo hi; sleep 20 & exit 4"]);
        let start = Instant::now();
        let c = capture(cmd, None).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{:?}",
            start.elapsed()
        );
        assert_eq!((c.stdout.as_slice(), c.code), (&b"hi\n"[..], Some(4)));
    }

    /// T280: a process still running at the limit is killed, and what it printed is kept.
    #[test]
    fn capture_kills_a_child_still_running_at_the_limit() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "echo 1.2.3; sleep 20"]);
        let start = Instant::now();
        let c = capture(cmd, Some(Duration::from_millis(300))).unwrap();
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
        assert_eq!((c.stdout.as_slice(), c.code), (&b"1.2.3\n"[..], None));
    }
}
