// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The MCP half of `doctor --fix` (T331.6): drop the chosen entries from a host's config. JSON and
//! JSONC go through the same byte-preserving editor as hooks; a TOML `[mcp_servers.<name>]` table
//! goes through `toml_edit`, which keeps every other byte. The result is parsed back and must hold
//! the old servers minus the removed ones, or it is not used.

use std::path::Path;

use rtok_mcp::config::{self, Fs as McpFs};
use rtok_mcp::spec::{Format, McpSpec};
use serde_json::{Map, Value};

use super::mcp_dupes::Loc;
use crate::agents::jsonc::{self, Seg};

/// One file's text, served to the `rtok_mcp` reader the doctor check also uses.
struct One<'a>(&'a Path, &'a str);

impl McpFs for One<'_> {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        (path == self.0).then(|| self.1.as_bytes().to_vec())
    }
    fn write(&mut self, _: &Path, _: Vec<u8>) -> anyhow::Result<()> {
        anyhow::bail!("the guard never writes")
    }
}

fn servers(text: &str, file: &Path, spec: &McpSpec) -> Option<Map<String, Value>> {
    config::read_servers(&One(file, text), spec).ok().flatten()
}

fn drop_entry(text: &str, file: &Path, l: &Loc) -> Option<String> {
    if l.spec.format == Format::Toml {
        let mut doc: toml_edit::DocumentMut = text.parse().ok()?;
        let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
        for key in &l.spec.key_path {
            table = table.get_mut(key)?.as_table_like_mut()?;
        }
        table.remove(&l.name)?;
        return Some(doc.to_string());
    }
    let keys = l.spec.key_path.iter().chain([&l.name]);
    let segs: Vec<Seg> = keys.map(|k| Seg::Key(k)).collect();
    let (next, found) = jsonc::remove_at(text, file, &segs).ok()?;
    found.then_some(next)
}

/// `value` without the object member at `keys`.
fn without(value: &mut Value, keys: &[&String]) {
    let Some((last, parents)) = keys.split_last() else {
        return;
    };
    let mut at = Some(value);
    for k in parents {
        at = at.and_then(|v| v.get_mut(k.as_str()));
    }
    if let Some(map) = at.and_then(Value::as_object_mut) {
        map.remove(last.as_str());
    }
}

/// `raw` without the entries `locs` name, or why the edit cannot be trusted.
pub(super) fn remove(raw: &str, file: &Path, locs: &[&Loc]) -> Result<String, &'static str> {
    let mut body = raw.to_string();
    for l in locs {
        body = drop_entry(&body, file, l).ok_or("an entry could not be located")?;
    }
    for l in locs {
        let (Some(mut want), Some(got)) =
            (servers(raw, file, &l.spec), servers(&body, file, &l.spec))
        else {
            return Err("the edit would change more than the duplicate entries");
        };
        for same in locs.iter().filter(|o| o.spec.key_path == l.spec.key_path) {
            want.remove(&same.name);
        }
        if want != got {
            return Err("the edit would change more than the duplicate entries");
        }
    }
    // The servers tables match; the rest of a JSON file must too. A TOML table is edited in place.
    if locs.iter().any(|l| l.spec.format != Format::Toml) {
        let (old, new) = (jsonc::parse(raw), jsonc::parse(&body));
        let (Ok(mut old), Ok(new)) = (old, new) else {
            return Err("the edit would change more than the duplicate entries");
        };
        for l in locs {
            without(
                &mut old,
                &l.spec.key_path.iter().chain([&l.name]).collect::<Vec<_>>(),
            );
        }
        if old != new {
            return Err("the edit would change more than the duplicate entries");
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn loc(name: &str) -> Loc {
        let agent = crate::agents::host("claude").expect("claude is a host");
        let kind = agent.variants()[0].kind;
        let spec = crate::agents::mcp::surfaces(agent, &Config::default(), kind)
            .remove(0)
            .spec;
        Loc {
            source: spec.config_path.display().to_string(),
            path: format!("mcpServers.{name}"),
            spec,
            name: name.into(),
        }
    }

    #[test]
    fn an_entry_that_is_not_there_is_refused() {
        let raw = r#"{"mcpServers": {"a": {"command": "x"}}}"#;
        let l = loc("b");
        assert_eq!(
            remove(raw, Path::new("/c.json"), &[&l]),
            Err("an entry could not be located")
        );
    }
}
