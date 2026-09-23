//! T178 / D32: `rtok hook --serve` answers what `rtok hook` prints, refuses a client whose
//! environment or version differs, runs once per home, and exits on a newer client or a deleted
//! home. Every resident here is this test's own child in its own temp home.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rtok_hook::Request;

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Home(PathBuf);

impl Home {
    /// `rtok` in this home with nothing else from the test's environment.
    fn rtok(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtok"));
        cmd.args(args)
            .env_clear()
            .envs(self.env())
            .current_dir(&self.0);
        for k in ["PATH", "SystemRoot"] {
            std::env::var_os(k).map(|v| cmd.env(k, v));
        }
        cmd
    }

    fn env(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        let dir = self.0.clone().into_os_string();
        vec![("RTOK_HOME".into(), dir.clone()), ("HOME".into(), dir)]
    }

    fn payload(&self) -> Vec<u8> {
        let tool_input = serde_json::json!({"command": "git status"});
        serde_json::json!({"hook_event_name": "PreToolUse", "session_id": "t178", "cwd": self.0,
            "tool_name": "Bash", "tool_input": tool_input})
        .to_string()
        .into_bytes()
    }

    fn call(&self, version: &str, fingerprint: u64) -> Option<Option<Vec<u8>>> {
        let endpoint = rtok_hook::endpoint(&self.0).expect("endpoint");
        #[cfg(unix)]
        let mut s = std::os::unix::net::UnixStream::connect(endpoint).ok()?;
        #[cfg(windows)]
        let mut s = std::fs::File::options()
            .read(true)
            .write(true)
            .open(endpoint)
            .ok()?;
        let (event, host, stdin) = ("PreToolUse".into(), String::new(), self.payload());
        let cwd = self.0.to_str().unwrap().into();
        let req = Request {
            version: version.into(),
            fingerprint,
            event,
            host,
            cwd,
            stdin,
        };
        s.write_all(&req.encode()).ok()?;
        rtok_hook::decode_response(&rtok_hook::read_frame(&mut s).ok()?)
    }

    /// A resident, once it answers (a refused probe).
    fn serve(&self) -> Child {
        let child = self.rtok(&["hook", "--serve"]).spawn().unwrap();
        let start = Instant::now();
        while self
            .call(VERSION, !rtok_hook::fingerprint(self.env()))
            .is_none()
        {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "resident never listened"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        child
    }
}

fn exits(child: &mut Child) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if child.try_wait().unwrap().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn the_resident_answers_like_rtok_hook_and_refuses_or_exits_otherwise() {
    // Short: a Unix socket path must fit 104 bytes.
    let home = Home(std::env::temp_dir().join(format!("rtr-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&home.0);
    std::fs::create_dir_all(&home.0).unwrap();
    let mut direct = home.rtok(&["hook", "PreToolUse"]);
    let mut direct = direct
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    direct
        .stdin
        .take()
        .unwrap()
        .write_all(&home.payload())
        .unwrap();
    let expected = direct.wait_with_output().unwrap().stdout;
    assert!(
        String::from_utf8_lossy(&expected).contains("rtok run"),
        "the control run rewrites"
    );

    let fp = rtok_hook::fingerprint(home.env());
    let mut resident = home.serve();
    assert_eq!(home.call(VERSION, fp), Some(Some(expected)));
    assert_eq!(
        home.call(VERSION, fp ^ 1),
        Some(None),
        "another environment is refused"
    );
    let second = home.rtok(&["hook", "--serve"]).status().unwrap();
    assert!(
        second.success(),
        "a second resident on the same home returns at once"
    );
    assert_eq!(
        home.call("999.0.0", fp),
        Some(None),
        "another version is refused"
    );
    assert!(exits(&mut resident), "and the stale resident exits");

    let mut resident = home.serve();
    // Windows keeps an open `hook.lock` from being deleted; there the test stops its
    // own child, waits for handles to release, then removes the home with a short retry
    // for ERROR_SHARING_VIOLATION (32).
    #[cfg(windows)]
    {
        resident.kill().unwrap();
        assert!(exits(&mut resident), "killed resident exits");
        let start = Instant::now();
        loop {
            match std::fs::remove_dir_all(&home.0) {
                Ok(()) => break,
                Err(e)
                    if e.raw_os_error() == Some(32) && start.elapsed() < Duration::from_secs(5) =>
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("remove home: {e}"),
            }
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_dir_all(&home.0).unwrap();
        assert!(
            exits(&mut resident),
            "a resident exits once its home is gone"
        );
    }
}
