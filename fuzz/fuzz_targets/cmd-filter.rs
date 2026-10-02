// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! The `cmd` plugin's stdout compactor (`rtok run`, `rtok filter --stdin`): a user `rules`
//! TOML file, the command's argv and its output. Also the T176 "already bounded" lexer.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

const BINS: &[&str] = &[
    "git", "cargo", "npm", "pnpm", "ls", "grep", "rg", "find", "kubectl", "gh", "docker", "pytest",
    "go", "tree", "sed", "tail", "head", "cat", "cd", "script", "skill",
];

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    rules: &'a str,
    fail_tail: u32,
    bin: u8,
    args: Vec<&'a str>,
    /// Hook shape: the whole Bash string as one argv word.
    one_word: bool,
    output: &'a str,
    exit: i32,
}

fuzz_target!(|i: Input<'_>| {
    let mut argv: Vec<String> = vec![BINS[usize::from(i.bin) % BINS.len()].to_string()];
    argv.extend(i.args.iter().take(16).map(|a| a.to_string()));
    if i.one_word {
        argv = vec![argv.join(" ")];
    }
    let _ = rtok::plugins::cmd::bounded::is_bounded(&argv.join(" "));
    rtok::fuzzing::cmd_filter(i.rules, i.fail_tail, &argv, i.output, i.exit);
});
