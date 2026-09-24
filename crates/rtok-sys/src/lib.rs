//! Cross-platform process and file-lock helpers for rtok.
//!
//! File locks go through [`fs4`] (safe API). Process helpers use rustix on Unix
//! and `windows-sys` on Windows (`unsafe_code = allow` lives only in this crate —
//! the main `rtok` package keeps `forbid`).

use std::fs::File;
use std::io;

/// Block until an exclusive lock is held on `file`.
pub fn lock_exclusive(file: &File) -> io::Result<()> {
    file.lock()
}

/// Non-blocking exclusive lock. `Ok(true)` acquired; `Ok(false)` held elsewhere.
pub fn try_lock_exclusive(file: &File) -> io::Result<bool> {
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(std::fs::TryLockError::WouldBlock) => Ok(false),
        Err(std::fs::TryLockError::Error(err)) => Err(err),
    }
}

/// Release a previously acquired exclusive lock.
pub fn unlock(file: &File) -> io::Result<()> {
    file.unlock()
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

/// This process's parent pid, to notice being orphaned: it changes when the parent exits
/// and the kernel reparents us (to 1, or a subreaper). `None` on Windows, where a parent
/// pid is never updated and so says nothing about the parent still being there.
pub fn parent_pid() -> Option<i32> {
    #[cfg(unix)]
    {
        rustix::process::getppid().map(|p| p.as_raw_nonzero().get())
    }
    #[cfg(windows)]
    {
        None
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

/// Windows only: stop the next spawned child from inheriting this process's own stdio
/// handles. `CreateProcess` inherits every already-inheritable handle the caller holds once a
/// child is given any redirected stdio — not just the handles that child was assigned — so a
/// supervisor spawned with `Stdio::null()` still keeps alive whatever piped our own stdout or
/// stderr (a test harness's `Command::output()`, or any other parent that captured us). Held by
/// a process meant to outlive us, that pipe's write end never closes, so the reader waiting on
/// EOF (`output()`/`wait_with_output()`) blocks long after both it and we have exited — the
/// hang behind T83.3. On Unix the equivalent fds are close-on-exec by default, so this is a
/// no-op there.
pub fn stop_inheriting_own_stdio() {
    #[cfg(windows)]
    {
        win::stop_inheriting_std_handles();
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{
        CloseHandle, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, STILL_ACTIVE, SetHandleInformation,
    };
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };

    pub fn stop_inheriting_std_handles() {
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let handle = unsafe { GetStdHandle(id) };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                continue;
            }
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) };
        }
    }

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
