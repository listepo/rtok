//! Cross-platform process and file-lock helpers for rtok.
//!
//! File locks go through [`fs4`] (safe API). Process helpers use rustix on Unix
//! and `windows-sys` on Windows (`unsafe_code = allow` lives only in this crate —
//! the main `rtok` package keeps `forbid`).

use std::fs::File;
use std::io;

use fs4::fs_std::FileExt;

/// Block until an exclusive lock is held on `file`.
pub fn lock_exclusive(file: &File) -> io::Result<()> {
    FileExt::lock_exclusive(file)
}

/// Non-blocking exclusive lock. `Ok(true)` acquired; `Ok(false)` held elsewhere.
pub fn try_lock_exclusive(file: &File) -> io::Result<bool> {
    FileExt::try_lock_exclusive(file)
}

/// Release a previously acquired exclusive lock.
pub fn unlock(file: &File) -> io::Result<()> {
    FileExt::unlock(file)
}

/// True while `pid` names a live process.
pub fn process_alive(pid: i32) -> bool {
    #[cfg(unix)]
    {
        use rustix::process::Pid;
        (pid > 0)
            .then(|| Pid::from_raw(pid))
            .flatten()
            .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
    }
    #[cfg(windows)]
    {
        win::alive(pid)
    }
}

/// Ask a process to exit politely (Unix TERM) or terminate (Windows).
pub fn process_term(pid: i32) {
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal};
        if let Some(p) = (pid > 0).then(|| Pid::from_raw(pid)).flatten() {
            let _ = rustix::process::kill_process(p, Signal::TERM);
        }
    }
    #[cfg(windows)]
    {
        win::terminate(pid);
    }
}

/// Force-kill a process (Unix KILL / Windows TerminateProcess).
pub fn process_kill(pid: i32) {
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal};
        if let Some(p) = (pid > 0).then(|| Pid::from_raw(pid)).flatten() {
            let _ = rustix::process::kill_process(p, Signal::KILL);
        }
    }
    #[cfg(windows)]
    {
        win::terminate(pid);
    }
}

/// Become a session leader so closing the terminal does not take the tree down.
/// No-op on Windows.
pub fn setsid() {
    #[cfg(unix)]
    {
        let _ = rustix::process::setsid();
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };

    pub fn alive(pid: i32) -> bool {
        if pid <= 0 {
            return false;
        }
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32) };
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
        unsafe { CloseHandle(handle) };
        ok != 0 && code == STILL_ACTIVE as u32
    }

    pub fn terminate(pid: i32) {
        if pid <= 0 {
            return;
        }
        let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, pid as u32) };
        if handle.is_null() {
            return;
        }
        let _ = unsafe { TerminateProcess(handle, 1) };
        unsafe { CloseHandle(handle) };
    }
}
