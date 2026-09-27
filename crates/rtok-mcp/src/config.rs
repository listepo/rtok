//! Read, write and remove one MCP entry in a host's config file, whatever shape that host uses
//! (T277): strict JSON now; JSONC (comments and formatting kept) and TOML (formatting kept via
//! `toml_edit`) land in this crate's second commit. Every host walks the same dotted `key_path`;
//! the [`spec::Format`] is the only thing that changes what runs underneath.
//!
//! Per `rust.md`'s "Config files" rule: this is the one module that owns this I/O, and it is
//! deliberately schema-less — a foreign config gets no schema from rtok. It reads and writes
//! only `key_path.<server.name>` and leaves every other key exactly as it found it.
//!
//! All I/O goes through [`Fs`], so a test plugs in an in-memory map instead of the real disk
//! (this PR's own test does exactly that). The disk-backed `Fs` a host installer uses lands
//! with the first host (T277 PR 2) and must write atomically and back up before every change,
//! the same contract `rtok_agent_sdk::write_atomic` / `backup` already give real files.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::spec::{Format, McpSpec};

/// The disk (or in-memory stand-in) [`read_entry`]/[`write_entry`]/[`remove_entry`] go through.
/// A real implementation must write atomically and name the file in any parse error (`rust.md`);
/// `backup`'s default is a no-op, which is only valid for a throwaway or in-memory `Fs` — a
/// disk-backed one reuses `rtok_agent_sdk::backup` instead of a second copy of that logic.
pub trait Fs {
    /// Bytes at `path`, or `None` if it does not exist.
    fn read(&self, path: &Path) -> Option<Vec<u8>>;
    /// Overwrite (or create) `path` with `bytes`.
    fn write(&mut self, path: &Path, bytes: Vec<u8>) -> Result<()>;
    /// Best-effort copy of `path`'s current contents before it changes.
    fn backup(&mut self, _path: &Path) -> Result<()> {
        Ok(())
    }
}

/// Whether a write actually changed the file — the "no changes" gate every host installer keeps
/// today (`rtok_agent_sdk::NO_CHANGES`), now computed in one place instead of per host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Written {
    Changed,
    Unchanged,
}

/// The entry at `spec.key_path.<spec.server.name>`, or `None` if the file, the key, or the name
/// is absent — never a substring match on the raw text (the bug `src/agents/mcp.rs::has_entry`
/// already fixed for the read side; this is the same rule for every format).
pub fn read_entry(fs: &impl Fs, spec: &McpSpec) -> Result<Option<Value>> {
    match spec.format {
        Format::Json => read_json(fs, spec),
        Format::Jsonc | Format::Toml => not_yet(spec),
    }
}

/// Write `entry` at that path, keeping every other key as it was. `Unchanged` when the file
/// already holds exactly `entry` there.
pub fn write_entry(fs: &mut impl Fs, spec: &McpSpec, entry: &Value) -> Result<Written> {
    if read_entry(fs, spec)?.as_ref() == Some(entry) {
        return Ok(Written::Unchanged);
    }
    match spec.format {
        Format::Json => write_json(fs, spec, Some(entry)),
        Format::Jsonc | Format::Toml => not_yet(spec),
    }
}

/// Remove the entry. `Unchanged` when there was nothing to remove.
pub fn remove_entry(fs: &mut impl Fs, spec: &McpSpec) -> Result<Written> {
    if read_entry(fs, spec)?.is_none() {
        return Ok(Written::Unchanged);
    }
    match spec.format {
        Format::Json => write_json(fs, spec, None),
        Format::Jsonc | Format::Toml => not_yet(spec),
    }
}

/// `Format::Jsonc` and `Format::Toml` land in this crate's second commit; no `McpSpec` in this
/// commit uses either, so this only guards against a future spec landing ahead of its config
/// support.
fn not_yet<T>(spec: &McpSpec) -> Result<T> {
    bail!(
        "{:?} config not implemented yet (T277 PR1, second commit): {}",
        spec.format,
        spec.config_path.display()
    )
}

// ---- JSON (strict; T79 refuses to rewrite a file that fails to parse as plain JSON) ----

fn parse_json(bytes: &[u8], path: &Path) -> Result<Value> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_slice(bytes).with_context(|| format!("{}: not strict JSON", path.display()))
}

fn read_json(fs: &impl Fs, spec: &McpSpec) -> Result<Option<Value>> {
    let Some(bytes) = fs.read(&spec.config_path) else {
        return Ok(None);
    };
    let root = parse_json(&bytes, &spec.config_path)?;
    Ok(walk_json(&root, &spec.key_path)
        .and_then(|s| s.get(spec.server.name))
        .cloned())
}

fn walk_json<'a>(
    root: &'a Value,
    key_path: &[String],
) -> Option<&'a serde_json::Map<String, Value>> {
    key_path.iter().try_fold(root, |v, k| v.get(k))?.as_object()
}

fn write_json(fs: &mut impl Fs, spec: &McpSpec, entry: Option<&Value>) -> Result<Written> {
    let mut root = match fs.read(&spec.config_path) {
        Some(bytes) => parse_json(&bytes, &spec.config_path)?,
        None => Value::Object(Default::default()),
    };
    if !root.is_object() {
        root = Value::Object(Default::default());
    }
    set_json_at(&mut root, &spec.key_path, spec.server.name, entry);
    let mut body = serde_json::to_string_pretty(&root)?;
    body.push('\n');
    fs.backup(&spec.config_path)?;
    fs.write(&spec.config_path, body.into_bytes())?;
    Ok(Written::Changed)
}

/// Insert or remove `name` under the dotted `key_path` inside `node`: creates intermediate
/// objects for a write, and drops them again once they empty out for a remove, so a removed
/// host entry leaves the file exactly as it read before rtok ever touched it.
fn set_json_at(node: &mut Value, key_path: &[String], name: &str, entry: Option<&Value>) {
    let Some((head, rest)) = key_path.split_first() else {
        let obj = node.as_object_mut().expect("caller keeps this an object");
        match entry {
            Some(v) => {
                obj.insert(name.to_string(), v.clone());
            }
            None => {
                obj.remove(name);
            }
        }
        return;
    };
    let obj = node.as_object_mut().expect("caller keeps this an object");
    if entry.is_none() && !obj.contains_key(head) {
        return; // nothing to remove
    }
    let child = obj
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Default::default()));
    if !child.is_object() {
        *child = Value::Object(Default::default());
    }
    set_json_at(child, rest, name, entry);
    if entry.is_none() && child.as_object().is_some_and(serde_json::Map::is_empty) {
        obj.remove(head);
    }
}
