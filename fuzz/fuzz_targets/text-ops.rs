// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! Text transforms on untrusted content: terse prose compression (all intensities),
//! `rtok expand` line ranges / grep context / head-tail cut, the managed memory block splice
//! in agent instruction files, and the TOON table encoder over arbitrary JSON.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rtok::modes::{CaveIntensity, compress_prose};
use rtok::plugins::memory::sync;

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    text: &'a str,
    lines: Option<&'a str>,
    grep: Option<&'a str>,
    context: u8,
    max: u16,
    block: Option<&'a str>,
    json: &'a [u8],
    min_rows: u8,
}

fuzz_target!(|i: Input<'_>| {
    for level in [
        CaveIntensity::Lite,
        CaveIntensity::Full,
        CaveIntensity::Ultra,
    ] {
        let _ = compress_prose(i.text, level);
    }
    // A user regex: keep it short so one input cannot spend the run compiling it.
    let grep = i.grep.filter(|g| g.len() <= 64);
    rtok::fuzzing::expand(i.text, i.lines, grep, i.context, i.max);
    let _ = sync::has_block(i.text);
    if let Ok(spliced) = sync::splice(i.text, i.block) {
        let _ = sync::splice(&spliced, None);
    }
    if let Ok(value) = serde_json::from_slice(i.json) {
        rtok::fuzzing::toon(&value, i.min_rows);
    }
});
