//! `rtok-hook <event> [--host <name>]` (T178, D32): what a host runs per hook call in place of
//! `rtok hook`. It hands the call to the resident `rtok hook --serve` and prints its answer,
//! which skips rtok's own process start. When no resident answers, or one refuses, it runs
//! `rtok hook` itself. A resident that takes longer than [`TIMEOUT`] gets `{}` printed for it:
//! fail open, never a hung hook.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use rtok_hook::Request;

const TIMEOUT: Duration = Duration::from_millis(50);

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let home = rtok_hook::home(|k| std::env::var_os(k));
    let endpoint = home.as_deref().and_then(rtok_hook::endpoint);
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|d| d.into_os_string().into_string().ok());
    let host_var = rtok_hook::HOST_VARS
        .iter()
        .any(|k| std::env::var_os(k).is_some());
    let (Some((event, host)), Some(endpoint), Some(cwd), false) =
        (parse(&args), endpoint, cwd, host_var)
    else {
        return rtok(&args, None);
    };
    let Some(mut stream) = rtok_hook::connect(&endpoint) else {
        return rtok(&args, None);
    };
    let mut stdin = Vec::new();
    let _ = io::stdin().read_to_end(&mut stdin);
    let req = Request {
        version: env!("CARGO_PKG_VERSION").into(),
        fingerprint: rtok_hook::fingerprint(std::env::vars_os()),
        event,
        host,
        cwd,
        stdin,
    };
    let frame = req.encode();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(rtok_hook::exchange(&mut stream, &frame));
    });
    match rx.recv_timeout(TIMEOUT) {
        Ok(Some(Some(out))) => {
            let _ = io::stdout().write_all(&out);
            ExitCode::SUCCESS
        }
        Err(RecvTimeoutError::Timeout) => fail_open(),
        _ => rtok(&args, Some(&req.stdin)),
    }
}

/// `<event> [--host <name>]`; anything else is `rtok hook`'s to parse.
fn parse(args: &[OsString]) -> Option<(String, String)> {
    let args: Vec<&str> = args.iter().map(|a| a.to_str()).collect::<Option<_>>()?;
    match args[..] {
        [e] if !e.starts_with('-') => Some((e.into(), String::new())),
        [e, "--host", h] if !e.starts_with('-') => Some((e.into(), h.into())),
        _ => None,
    }
}

/// `rtok` beside this binary, else whichever one `PATH` finds.
fn rtok_exe() -> PathBuf {
    let name = format!("rtok{}", std::env::consts::EXE_SUFFIX);
    std::env::current_exe()
        .ok()
        .and_then(|p| Some(p.parent()?.join(&name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| name.into())
}

/// `rtok hook <args>` with its exit code. `stdin` is the payload when this process already read
/// it; otherwise `rtok` reads the host's stdin itself.
fn rtok(args: &[OsString], stdin: Option<&[u8]>) -> ExitCode {
    let mut cmd = Command::new(rtok_exe());
    cmd.arg("hook").args(args);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let Ok(mut child) = cmd.spawn() else {
        return fail_open();
    };
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = pipe.write_all(bytes);
    }
    match child.wait().ok().and_then(|s| s.code()) {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => fail_open(),
    }
}

/// What `rtok hook` prints when it cannot help: no change to the call.
fn fail_open() -> ExitCode {
    let _ = io::stdout().write_all(b"{}");
    ExitCode::SUCCESS
}
