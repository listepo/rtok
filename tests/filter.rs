//! T10.2: `rtok filter --cmd` reads stdin; OpenCode plugin mock.

use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn printf_git_status_returns_filtered_text() {
    let bin = env!("CARGO_BIN_EXE_rtok");
    let mut child = Command::new(bin)
        .args(["filter", "--cmd", "git status"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin
            .write_all(
                b"On branch main\nChanges not staged for commit:\n\tmodified:   src/lib.rs\n",
            )
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("On branch main"), "{s}");
    assert!(s.contains("modified:   src/lib.rs"), "{s}");
    assert!(!s.contains("Changes not staged"), "{s}");
}

#[test]
fn opencode_plugin_unit_test_with_api_mock() {
    let status = Command::new("node")
        .args([
            "--experimental-strip-types",
            "--disable-warning=ExperimentalWarning",
            "--test",
            "hosts/opencode/rtok.test.ts",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("node");
    assert!(status.success());
}
