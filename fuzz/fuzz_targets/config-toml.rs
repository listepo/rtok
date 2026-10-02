// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! `~/.rtok/config.toml` text through `rtok config validate` and the file-free config stack
//! (defaults < user TOML < legacy-key fold, `~` expansion, `config show` rows).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    rtok::fuzzing::config_toml(text);
});
