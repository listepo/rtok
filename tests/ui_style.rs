// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! `[ui]` emoji and colour against the real binary. Every run here is on pipes, which is how
//! hooks, agents and CI see rtok: plain bytes unless colour is forced, and never an emoji.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const ESC: char = '\u{1b}';
const EMOJI: [&str; 4] = ["✅", "💡", "⚠️", "❌"];

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-ui-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    // `config validate` checks a file; it does not create one.
    fs::write(dir.join("config.toml"), "").unwrap();
    dir
}

/// `rtok <args>` with the colour environment cleared, then `env` on top.
fn rtok(home: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtok"));
    cmd.args(args).env("RTOK_HOME", home).env("HOME", home);
    for var in [
        "NO_COLOR",
        "FORCE_COLOR",
        "CLICOLOR",
        "CLICOLOR_FORCE",
        "TERM",
        "RTOK_UI_EMOJI",
        "RTOK_UI_COLOR",
        "RTOK_CONFIG",
    ] {
        cmd.env_remove(var);
    }
    cmd.envs(env.iter().copied()).output().unwrap()
}

fn stdout(out: &Output) -> String {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn no_emoji(text: &str) {
    for e in EMOJI {
        assert!(!text.contains(e), "no emoji off a terminal: {text:?}");
    }
}

#[test]
fn a_pipe_gets_the_bare_line() {
    let home = home("pipe");
    let out = stdout(&rtok(&home, &["config", "validate"], &[]));
    assert_eq!(out, format!("ok {}\n", home.join("config.toml").display()));
    // NO_COLOR on a pipe changes nothing: it was already plain.
    assert_eq!(
        stdout(&rtok(&home, &["config", "validate"], &[("NO_COLOR", "1")])),
        out
    );
}

#[test]
fn an_error_on_a_pipe_keeps_std_s_bytes() {
    let home = home("error");
    let out = rtok(&home, &["config", "get", "no.such.key"], &[]);
    assert!(!out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "Error: unknown key: no.such.key\n"
    );
}

#[test]
fn clicolor_force_paints_a_pipe_but_adds_no_emoji() {
    let home = home("force");
    let out = stdout(&rtok(
        &home,
        &["config", "validate"],
        &[("CLICOLOR_FORCE", "1")],
    ));
    assert!(out.starts_with(&format!("{ESC}[32mok ")), "{out:?}");
    no_emoji(&out);
}

#[test]
fn the_color_key_wins_over_clicolor_force() {
    let home = home("key");
    let force = [("CLICOLOR_FORCE", "1"), ("RTOK_UI_COLOR", "false")];
    let out = stdout(&rtok(&home, &["config", "validate"], &force));
    assert!(!out.contains(ESC), "RTOK_UI_COLOR=false: {out:?}");
    // The same through the file: `[ui] color = false`.
    let cfg = home.join("config.toml");
    fs::write(&cfg, "[ui]\ncolor = false\n").unwrap();
    let out = stdout(&rtok(&home, &["config", "validate"], &force[..1]));
    assert_eq!(out, format!("ok {}\n", cfg.display()));
    no_emoji(&out);
}
