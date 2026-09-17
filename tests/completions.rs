//! T53.2: `rtok completions <shell>` renders every supported shell and `rtok man`
//! prints a roff page. Snapshots stay out of here: the man page embeds the build
//! version (git sha), and one shell's completions are snapshotted in
//! `tests/trycmd/completions-bash.toml` instead.

use assert_cmd::Command;
use clap::ValueEnum;
use clap_complete::Shell;

fn stdout(args: &[&str]) -> Vec<u8> {
    Command::cargo_bin("rtok")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone()
}

#[test]
fn every_shell_renders_non_empty_completions() {
    for shell in Shell::value_variants() {
        let name = shell
            .to_possible_value()
            .expect("shell has a value")
            .get_name()
            .to_string();
        let out = stdout(&["completions", &name]);
        let text = String::from_utf8(out).expect("completions are utf-8");
        assert!(!text.trim().is_empty(), "{name} completions are empty");
        assert!(
            text.contains("rtok"),
            "{name} completions never name the binary"
        );
    }
}

#[test]
fn unknown_shell_is_refused() {
    Command::cargo_bin("rtok")
        .unwrap()
        .args(["completions", "tcsh"])
        .assert()
        .failure();
}

#[test]
fn man_prints_a_roff_page() {
    let text = String::from_utf8(stdout(&["man"])).expect("man is utf-8");
    assert!(text.contains(".TH"), "roff title header, got:\n{text}");
    assert!(
        text.to_lowercase().contains("rtok"),
        "the page names the binary"
    );
    assert!(
        text.contains("completions"),
        "the page documents the new command"
    );
}
