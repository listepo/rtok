//! Codex installer (`rtok setup codex`, plan T10.3).
//!
//! Codex reads MCP servers from `~/.codex/config.toml` as `[mcp_servers.<name>]`
//! tables with `command` and `args`. It has no shell hooks, so MCP plus proxy
//! wiring (T11.5) is the whole install. Edits go through `toml_edit` so the
//! user's comments and other servers survive.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use super::claude::backup;
use crate::config::Config;

const NAME: &str = "rtok";
const BLOCK: &str = "[mcp_servers.rtok]\ncommand = \"rtok\"\nargs = [\"mcp\"]";

/// Apply, dry-run, or remove the `[mcp_servers.rtok]` block.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.codex.config_path;
    let mut doc = load(path)?;
    let report = if remove {
        strip_ours(&mut doc)
    } else {
        insert_ours(&mut doc, path.display())?
    };
    persist(cfg, path, &doc, &report)?;
    Ok(report)
}

/// Point Codex `model_provider` at this proxy (T11.5).
pub fn register_proxy(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.codex.config_path;
    let mut doc = load(path)?;
    let url = super::openai_proxy_url(cfg);
    let report = if remove {
        strip_proxy(&mut doc)
    } else {
        insert_proxy(&mut doc, &url)?
    };
    persist(cfg, path, &doc, &report)?;
    Ok(report)
}

fn load(path: &Path) -> Result<DocumentMut> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .parse::<DocumentMut>()
        .with_context(|| path.display().to_string())
}

fn persist(cfg: &Config, path: &Path, doc: &DocumentMut, report: &str) -> Result<()> {
    if cfg.setup.dry_run || report == "no changes" {
        return Ok(());
    }
    if cfg.setup.backup {
        backup(path)?;
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).ok();
    }
    fs::write(path, doc.to_string()).with_context(|| path.display().to_string())?;
    Ok(())
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

fn insert_proxy(doc: &mut DocumentMut, url: &str) -> Result<String> {
    let old = doc
        .get("model_provider")
        .and_then(Item::as_str)
        .map(str::to_string);
    let ours = doc
        .get("model_providers")
        .and_then(Item::as_table)
        .and_then(|t| t.get(NAME))
        .and_then(Item::as_table)
        .is_some_and(|t| {
            t.get("name").and_then(Item::as_str) == Some(NAME)
                && t.get("base_url").and_then(Item::as_str) == Some(url)
        });
    if old.as_deref() == Some(NAME) && ours {
        return Ok("no changes".into());
    }
    doc["model_provider"] = value(NAME);
    let tables = doc
        .entry("model_providers")
        .or_insert(Item::Table(Table::new()));
    let Some(tables) = tables.as_table_mut() else {
        bail!("model_providers is not a table");
    };
    tables.set_implicit(true);
    let mut entry = Table::new();
    entry["name"] = value(NAME);
    entry["base_url"] = value(url);
    tables.insert(NAME, Item::Table(entry));
    let revert = match old.as_deref() {
        Some(s) if s != NAME => format!("revert: set model_provider to {s}"),
        _ => "revert: remove [model_providers.rtok]".into(),
    };
    Ok(format!(
        "+ [model_providers.rtok]\nbase_url = \"{url}\"\n{revert}"
    ))
}

fn strip_proxy(doc: &mut DocumentMut) -> String {
    let key = doc.get("model_provider").and_then(Item::as_str) == Some(NAME);
    if key {
        doc.remove("model_provider");
    }
    let table = doc
        .get_mut("model_providers")
        .and_then(Item::as_table_mut)
        .and_then(|t| t.remove(NAME))
        .is_some();
    if key || table {
        "- [model_providers.rtok]".into()
    } else {
        "no changes".into()
    }
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

    #[test]
    fn proxy_dry_run_shows_one_change_and_touches_nothing() {
        let (c, path) = cfg("proxy-dry", true);
        fs::write(&path, "# keep me\n").unwrap();
        let out = register_proxy(&c, false).unwrap();
        assert_eq!(out.matches("+ [model_providers.rtok]").count(), 1);
        assert!(out.contains("8790/v1"), "{out}");
        assert!(out.contains("revert:"), "{out}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "# keep me\n");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn proxy_apply_is_idempotent_and_remove_strips() {
        let (c, path) = cfg("proxy-apply", false);
        fs::write(&path, "# keep me\nmodel = \"o3\"\n").unwrap();
        let first = register_proxy(&c, false).unwrap();
        assert!(first.contains("[model_providers.rtok]"), "{first}");
        assert_eq!(register_proxy(&c, false).unwrap(), "no changes");
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("model_provider = \"rtok\""), "{raw}");
        assert!(raw.contains("base_url"), "{raw}");
        assert!(raw.contains("# keep me"), "{raw}");
        assert_eq!(
            register_proxy(&c, true).unwrap(),
            "- [model_providers.rtok]"
        );
        assert_eq!(register_proxy(&c, true).unwrap(), "no changes");
        let gone = fs::read_to_string(&path).unwrap();
        assert!(!gone.contains("model_providers.rtok"), "{gone}");
        assert!(gone.contains("# keep me"), "{gone}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
