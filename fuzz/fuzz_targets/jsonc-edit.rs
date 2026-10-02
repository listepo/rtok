// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! The JSONC surgical editor that installs rtok into host settings (Zed `context_servers`,
//! VS Code `chat.pluginLocations`). Oracle: an edit of a file that parses must still parse,
//! carry the entry it wrote, and a remove must leave a parseable file without it.
#![no_main]

use std::path::Path;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rtok::agents::jsonc;
use serde_json::json;

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    raw: &'a str,
    top: &'a str,
    key: &'a str,
    arg: &'a str,
}

fuzz_target!(|i: Input<'_>| {
    let path = Path::new("settings.json");
    let _ = jsonc::strip_comments(i.raw);
    let entry = json!({"command": "rtok", "args": ["mcp", i.arg]});
    let Ok((body, _)) = jsonc::upsert_member(i.raw, path, i.top, i.key, &entry) else {
        return;
    };
    let parsed = jsonc::parse(&body)
        .unwrap_or_else(|e| panic!("upsert produced invalid JSONC ({e}):\n{body}"));
    assert_eq!(
        parsed.get(i.top).and_then(|t| t.get(i.key)),
        Some(&entry),
        "upsert did not land the entry:\n{body}"
    );
    let (back, removed) = jsonc::remove_member(&body, path, i.top, i.key)
        .unwrap_or_else(|e| panic!("remove failed on our own output ({e}):\n{body}"));
    assert!(removed, "remove missed the entry upsert wrote:\n{body}");
    let after = jsonc::parse(&back)
        .unwrap_or_else(|e| panic!("remove produced invalid JSONC ({e}):\n{back}"));
    let still = after.get(i.top).and_then(|t| t.get(i.key));
    assert!(still.is_none(), "remove left the entry:\n{back}");
});
