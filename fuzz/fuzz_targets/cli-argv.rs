//! Arbitrary argv through the real clap tree: `rtok::cli::Cli::try_parse_from`, plus the
//! error/help rendering a failed parse prints. Tokens are drawn mostly from the tree's own
//! subcommand names, flags and possible values so the fuzzer gets past the first word.
#![no_main]

use std::ffi::OsString;
use std::sync::OnceLock;

use arbitrary::Arbitrary;
use clap::{Command, CommandFactory, Parser};
use libfuzzer_sys::fuzz_target;
use rtok::cli::Cli;

#[derive(Arbitrary, Debug)]
enum Tok {
    /// A word of the clap tree (subcommand, alias, `--long`, `-s`, possible value).
    Known(u16),
    /// `--long=value` / `-svalue` glued forms.
    Glued(u16, String),
    Free(String),
    Num(i64),
    /// Non-UTF-8 argv, which a Unix shell can pass.
    Bytes(Vec<u8>),
}

fn vocab() -> &'static [String] {
    static WORDS: OnceLock<Vec<String>> = OnceLock::new();
    WORDS.get_or_init(|| {
        let cmd = Cli::command();
        // Clap's own consistency checks (conflicts, required groups, duplicate flags).
        cmd.clone().debug_assert();
        let mut words: Vec<String> = ["--", "-", "--help", "-h", "--version", "-V", "help"]
            .map(String::from)
            .to_vec();
        walk(&cmd, &mut words);
        words.sort();
        words.dedup();
        words
    })
}

fn walk(cmd: &Command, words: &mut Vec<String>) {
    for arg in cmd.get_arguments() {
        if let Some(long) = arg.get_long() {
            words.push(format!("--{long}"));
        }
        if let Some(short) = arg.get_short() {
            words.push(format!("-{short}"));
        }
        for value in arg.get_possible_values() {
            words.push(value.get_name().to_string());
        }
    }
    for sub in cmd.get_subcommands() {
        words.push(sub.get_name().to_string());
        words.extend(sub.get_all_aliases().map(String::from));
        walk(sub, words);
    }
}

fn to_os(tok: Tok, words: &[String]) -> OsString {
    let word = |i: u16| words[usize::from(i) % words.len()].clone();
    match tok {
        Tok::Known(i) => word(i).into(),
        Tok::Glued(i, v) => {
            let w = word(i);
            if w.starts_with("--") {
                format!("{w}={v}")
            } else {
                format!("{w}{v}")
            }
            .into()
        }
        Tok::Free(s) => s.into(),
        Tok::Num(n) => n.to_string().into(),
        #[cfg(unix)]
        Tok::Bytes(b) => std::os::unix::ffi::OsStringExt::from_vec(b),
        #[cfg(not(unix))]
        Tok::Bytes(b) => String::from_utf8_lossy(&b).into_owned().into(),
    }
}

fuzz_target!(|toks: Vec<Tok>| {
    let words = vocab();
    let argv = std::iter::once(OsString::from("rtok"))
        .chain(toks.into_iter().take(32).map(|t| to_os(t, words)));
    if let Err(e) = Cli::try_parse_from(argv) {
        let _ = e.render().to_string();
    }
});
