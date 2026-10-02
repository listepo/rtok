// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! Entry points for the `fuzz/` cargo-fuzz targets, compiled only under `--cfg fuzzing`
//! (set by `cargo fuzz`). They reach crate-private parsers without touching the disk, the
//! environment or the log: the normal build never sees this module. Each function takes
//! untrusted input and must never panic; the targets under `fuzz/fuzz_targets/` drive them.

use std::path::Path;

use serde_json::Value;

use crate::config::{layers, validate};
use crate::plugins::cmd::{formatters, rules};

/// `rtok config validate` over `text`, then the file-free part of the config stack
/// (defaults < `text` < legacy fold, `~` expansion) and the `config show` rows.
pub fn config_toml(text: &str) {
    let _ = validate::issues_in(Path::new("config.toml"), text);
    let _ = layers::extract_str(text);
}

/// `[rule]` TOML (`~/.rtok/rules`) merged over the built-ins, then the stdout compactor
/// for `argv` with both the built-in and the merged settings.
pub fn cmd_filter(rules_toml: &str, fail_tail: u32, argv: &[String], output: &str, exit: i32) {
    let id = "0123456789abcdef";
    let _ = formatters::compress(&rules::Settings::builtin(), argv, output, exit, id);
    if let Ok(user) = rules::parse_strict(rules_toml) {
        let settings = rules::Settings::with_user_rules(user, fail_tail);
        let _ = formatters::compress(&settings, argv, output, exit, id);
    }
}

/// `rtok expand` line ranges, grep with context, and the head/tail `cut`.
pub fn expand(text: &str, lines: Option<&str>, grep: Option<&str>, context: u8, max: u16) {
    let _ = crate::expand::filter_lines(text, lines, grep, usize::from(context));
    if let Some(spec) = lines {
        let _ = crate::expand::parse_range(spec, text.lines().count());
    }
    let _ = crate::expand::cut(text, "[rtok expand 0123]", usize::from(max));
}

/// The TOON encoder over any JSON value that passes its own tabular gate.
pub fn toon(value: &Value, min_rows: u8) {
    use crate::plugins::toon::{encode, tabular_keys};
    if let Some(keys) = tabular_keys(value, usize::from(min_rows))
        && let Some(rows) = value.as_array()
    {
        let _ = encode(rows, &keys);
    }
}

/// `read` outlines and comment stripping for `src` as a file named `name`.
pub fn outline(name: &str, src: &str, mode: &str) {
    use crate::plugins::read::outline;
    let path = Path::new(name);
    let _ = outline::render(path, src, mode);
    let _ = outline::stripped(path, src);
}

/// Host tool name → Claude tool name, and the guard's duplicate key for that call.
pub fn tool_names(tool: &str, input: &Value) -> Option<String> {
    let canonical = crate::hooks::types::canonical_tool_name(tool);
    crate::plugins::guard::cache_key(&canonical, input, None)
}
