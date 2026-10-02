// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Session transcript ingest behind `rtok stats` (`measure::jsonl`): arbitrary JSONL lines
//! as Claude Code writes them, malformed and non-UTF-8-adjacent input included.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let _ = rtok::measure::jsonl::parse_jsonl(text);
});
