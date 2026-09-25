//! T178 / D32: `rtok hook --serve`, the optional resident hook process. The `rtok-hook` client
//! sends it each hook call over a Unix socket (Windows: a named pipe) and prints what it answers
//! — what `rtok hook` would print, without a process start. One per home (`hook.lock`). Calls
//! run one at a time in the client's cwd, because plugins read the process cwd. It refuses a
//! client whose environment would load another config, and exits when its home goes or a client
//! of another version calls. Nothing depends on it: a refused or absent resident means the
//! client runs `rtok hook` as before.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use anyhow::Result;
use rtok_hook::Request;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Notify;

use crate::config::Config;

struct State {
    fingerprint: u64,
    /// One call at a time: each one sets the process cwd.
    turn: Mutex<()>,
    /// A client of another version called: this binary is stale.
    stale: Notify,
}

/// Serve until the home goes away or a client of another version calls. Returns at once when
/// another resident holds the home or the environment gives it no endpoint.
pub fn serve() -> Result<()> {
    let Some(home) = rtok_hook::home(|k| std::env::var_os(k)) else {
        return Ok(());
    };
    let Some(endpoint) = rtok_hook::endpoint(&home) else {
        return Ok(());
    };
    std::fs::create_dir_all(&home)?;
    let lock_path = home.join("hook.lock");
    let lock = std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)?;
    if !rtok_sys::try_lock_exclusive(&lock)? {
        return Ok(());
    }
    let state = Arc::new(State {
        fingerprint: rtok_hook::fingerprint(std::env::vars_os()),
        turn: Mutex::new(()),
        stale: Notify::new(),
    });
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(listen(&endpoint, &lock_path, state))
}

#[cfg(unix)]
async fn listen(endpoint: &Path, lock: &Path, state: Arc<State>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let _ = tokio::fs::remove_file(endpoint).await;
    let listener = tokio::net::UnixListener::bind(endpoint)?;
    tokio::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600)).await?;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            conn = listener.accept() => if let Ok((stream, _)) = conn {
                tokio::spawn(answer(stream, state.clone()));
            },
            () = state.stale.notified() => break,
            _ = tick.tick() => if !lock.exists() || !endpoint.exists() { break },
        }
    }
    let _ = tokio::fs::remove_file(endpoint).await;
    Ok(())
}

#[cfg(windows)]
async fn listen(endpoint: &Path, lock: &Path, state: Arc<State>) -> Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = endpoint.as_os_str();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(name)?;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            conn = server.connect() => {
                let next = ServerOptions::new().create(name)?;
                let pipe = std::mem::replace(&mut server, next);
                if conn.is_ok() {
                    tokio::spawn(answer(pipe, state.clone()));
                }
            },
            () = state.stale.notified() => break,
            _ = tick.tick() => if !lock.exists() { break },
        }
    }
    Ok(())
}

/// Read one request, answer it, then close. Anything unexpected is a refusal.
async fn answer<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S, state: Arc<State>) {
    let req = read_request(&mut stream).await;
    let stale = req
        .as_ref()
        .is_some_and(|r| r.version != env!("CARGO_PKG_VERSION"));
    let out = match req {
        Some(req) if !stale && req.fingerprint == state.fingerprint => {
            let state = state.clone();
            tokio::task::spawn_blocking(move || state.run(req))
                .await
                .ok()
                .flatten()
        }
        _ => None,
    };
    let _ = stream
        .write_all(&rtok_hook::encode_response(out.as_deref()))
        .await;
    let _ = stream.flush().await;
    if stale {
        state.stale.notify_one();
    }
}

async fn read_request<S: AsyncRead + Unpin>(stream: &mut S) -> Option<Request> {
    let mut header = [0; 4];
    stream.read_exact(&mut header).await.ok()?;
    let mut body = vec![0; rtok_hook::frame_len(header).ok()?];
    stream.read_exact(&mut body).await.ok()?;
    Request::decode(&body)
}

impl State {
    /// What `rtok hook <event> [--host]` prints, run in the client's cwd.
    fn run(&self, req: Request) -> Option<Vec<u8>> {
        let _turn = self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        std::env::set_current_dir(PathBuf::from(&req.cwd)).ok()?;
        let host = (!req.host.is_empty()).then_some(req.host);
        let mut cfg = Config::load_lenient(None, crate::cli::hook_host_flag(host));
        // This process's env came from whichever call started it; the payload names the session.
        cfg.core.session_env.clear();
        let mut out = Vec::new();
        super::run(&req.event, &req.stdin[..], &mut out, &cfg);
        Some(out)
    }
}
