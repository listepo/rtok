//! Codex installer (`rtok setup codex`, plan T10.3).
//!
//! Codex reads MCP servers from `~/.codex/config.toml` as `[mcp_servers.<name>]`
//! tables with `command` and `args`. It has no shell hooks, so this is the whole
//! install; proxy wiring for Codex is T11.5. Edits go through `toml_edit` so the
//! user's comments and other servers survive.

use std::fs;

use anyhow::{Context, Result, bail};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use super::claude::backup;
use crate::config::Config;

const NAME: &str = "rtok";
const BLOCK: &str = "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]";

/// Apply, dry-run, or remove the `[mcp_servers.rtok]` block.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.codex.config_path;
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| path.display().to_string())?;
    let report = if remove {
        strip_ours(&mut doc)
    } else {
        insert_ours(&mut doc, path.display())?
    };
    if !cfg.setup.dry_run && report != "no changes" {
        if cfg.setup.backup {
            backup(path)?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).ok();
        }
        fs::write(path, doc.to_string()).with_context(|| path.display().to_string())?;
    }
    Ok(report)
}

fn insert_ours(doc: &mut DocumentMut, path: impl std::fmt::Display) -> Result<String> {
    let servers = doc
        .entry("mcp_servers")
        .or_insert(Item::Table(Table::new()));
    let Some(servers) = servers.as_table_mut() else {
        bail!("mcp_servers is not a table in {path}");
    };
    // No bare `[mcp_servers]` header: Codex's own files only carry the per-server tables.
    servers.set_implicit(true);
    if servers
        .get(NAME)
        .and_then(Item::as_table)
        .is_some_and(is_ours)
    {
        return Ok("no changes".into());
    }
    let mut entry = Table::new();
    entry["command"] = value(NAME);
    let mut args = Array::new();
    args.push("mcp");
    entry["args"] = value(args);
    servers.insert(NAME, Item::Table(entry));
    Ok(format!("+ {BLOCK}"))
}

fn strip_ours(doc: &mut DocumentMut) -> String {
    let removed = doc
        .get_mut("mcp_servers")
        .and_then(Item::as_table_mut)
        .and_then(|t| t.remove(NAME))
        .is_some();
    if removed {
        "- [mcp_servers.rtok]".into()
    } else {
        "no changes".into()
    }
}

fn is_ours(t: &Table) -> bool {
    let args: Vec<&str> = t
        .get("args")
        .and_then(Item::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .collect();
    t.get("command").and_then(Item::as_str) == Some(NAME) && args == ["mcp"]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg(dir: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-codex-{dir}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        let mut c = Config::default();
        c.setup.codex.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_shows_one_block_and_touches_nothing() {
        let (c, path) = cfg("dry", true);
        fs::write(&path, "model = \"o3\" # keep me\n").unwrap();
        let out = run(&c, false).unwrap();
        assert_eq!(out, format!("+ {BLOCK}"));
        assert_eq!(out.matches("[mcp_servers.rtok]").count(), 1);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "model = \"o3\" # keep me\n"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_keeps_comments_and_other_servers_and_is_idempotent() {
        let (c, path) = cfg("apply", false);
        fs::write(
            &path,
            "# codex config\nmodel = \"o3\"\n\n[mcp_servers.other]\ncommand = \"x\"\n",
        )
        .unwrap();
        assert!(run(&c, false).unwrap().starts_with("+ [mcp_servers.rtok]"));
        assert_eq!(run(&c, false).unwrap(), "no changes");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("# codex config\nmodel = \"o3\"\n"), "{raw}");
        assert!(
            raw.contains("[mcp_servers.other]\ncommand = \"x\"\n"),
            "{raw}"
        );
        assert!(raw.contains(BLOCK), "{raw}");
        assert!(!raw.contains("\n[mcp_servers]\n"), "{raw}");
        let parsed: toml_edit::DocumentMut = raw.parse().unwrap();
        assert_eq!(
            parsed["mcp_servers"]["rtok"]["args"][0].as_str(),
            Some("mcp")
        );
        assert_eq!(run(&c, true).unwrap(), "- [mcp_servers.rtok]");
        assert_eq!(run(&c, true).unwrap(), "no changes");
        assert!(!fs::read_to_string(&path).unwrap().contains("rtok"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_file_is_created_on_apply() {
        let (c, path) = cfg("new", false);
        run(&c, false).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), format!("{BLOCK}\n"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
