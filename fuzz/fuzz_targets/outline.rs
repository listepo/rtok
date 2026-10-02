// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `read` outlines (`map` / `signatures`) and comment stripping: tree-sitter tag queries
//! plus rtok's own post-processing, and the markdown heading scanner, over arbitrary source.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

const NAMES: &[&str] = &[
    "a.rs", "a.ts", "a.tsx", "a.js", "a.py", "a.dart", "a.c", "a.h", "a.go", "a.java", "a.kt",
    "a.swift", "a.cs", "a.rb", "a.php", "a.md", "a.mdx", "a.txt",
];
const MODES: &[&str] = &["map", "signatures", "full"];

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    name: u8,
    mode: u8,
    src: &'a str,
}

fuzz_target!(|i: Input<'_>| {
    let name = NAMES[usize::from(i.name) % NAMES.len()];
    let mode = MODES[usize::from(i.mode) % MODES.len()];
    rtok::fuzzing::outline(name, i.src, mode);
});
