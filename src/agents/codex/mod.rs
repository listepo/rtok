//! Codex installer (`rtok agents install codex`, plan T10.3).
//!
//! Codex reads MCP servers from `~/.codex/config.toml` as `[mcp_servers.<name>]`
//! tables with `command` and `args`, and lifecycle hooks from `hooks.json` (or
//! inline `[hooks]`). Compaction events `PreCompact`/`PostCompact` (T58.2) plus
//! MCP and proxy wiring (T11.5) are the install. Edits go through `toml_edit` so
//! the user's comments and other servers survive.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rtok_agent_sdk::{KETCH_INSTALL, NO_CHANGES, edit_json, object_at};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

const NAME: &str = "rtok";

const COMPACT: &[(&str, &str)] = &[("PreCompact", ""), ("PostCompact", "")];

/// The plugin tree (T121) and its id in the one-plugin marketplace `listepo/rtok` also is.
const PLUGIN_SRC: &str = "plugins/codex";
const PLUGIN_ID: &str = "rtok@rtok";

/// GitHub `owner/repo` shorthand `codex plugin marketplace add` resolves (T140): the repo
/// root's `.agents/plugins/marketplace.json` names this one marketplace `rtok`, whose only
/// plugin is `./plugins/codex` (relative to the repo root, not the marketplace file).
const MARKETPLACE_REPO: &str = "listepo/rtok";

/// What `codex plugin marketplace add {MARKETPLACE_REPO}` records in `~/.codex/config.toml`
/// under `[marketplaces.rtok]` — verified empirically against codex-cli 0.155.1 in a scratch
/// `CODEX_HOME` (2026-09-22): `source_type = "git"`, `source =
/// "https://github.com/listepo/rtok.git"`.
const MARKETPLACE_GIT_SOURCE: &str = "https://github.com/listepo/rtok.git";

/// Sibling of `config.toml`: `~/.codex/hooks.json` (Codex also reads inline `[hooks]`).
pub fn hooks_path(cfg: &Config) -> std::path::PathBuf {
    cfg.setup.codex.config_path.with_file_name("hooks.json")
}

/// Register `PreCompact` → `pre_compact` and `PostCompact` → `session_start` source=compact.
pub fn run_hooks(cfg: &Config, remove: bool) -> Result<String> {
    edit_json(&apply(cfg), &hooks_path(cfg), |root| {
        if remove {
            super::claude::strip_ours(root.get_mut("hooks"))
        } else {
            super::claude::insert_ours(
                object_at(root, "hooks"),
                COMPACT,
                &super::rtok_hook_bin(),
                "timeout",
                cfg.setup.hook_timeout_s,
            )
        }
    })
}

/// Codex's config dir: the one `config_path` lives in (`~/.codex`).
fn config_dir(cfg: &Config) -> PathBuf {
    let p = &cfg.setup.codex.config_path;
    p.parent().map(PathBuf::from).unwrap_or_else(|| p.clone())
}

/// One `codex plugin …` call; `CODEX_HOME` only when `config_path` is not the default.
fn codex_cli(cfg: &Config, args: &[&str]) -> std::result::Result<(), String> {
    let dir = config_dir(cfg);
    let default = super::home_dir().join(".codex");
    let env = (dir != default).then_some(("CODEX_HOME", dir.as_path()));
    super::run_cli("codex", args, env)
}

/// True when Codex's own config already enables `rtok@rtok`. Unlike Claude Code's separate
/// `installed_plugins.json`, `codex plugin add` writes this straight into
/// `~/.codex/config.toml` as `[plugins."rtok@rtok"]` `enabled = true` — verified empirically
/// against codex-cli 0.155.1 in a scratch `CODEX_HOME` (2026-09-22), since the docs at
/// developers.openai.com/plugins/build/plugins do not name `codex plugin add`/`remove`/`list`
/// at all (only `marketplace add|list|upgrade|remove`), yet `codex plugin add --help` on the
/// installed CLI shows they exist and work exactly like Claude's `plugin install`/`uninstall`.
pub(super) fn plugin_installed(cfg: &Config) -> bool {
    load(&cfg.setup.codex.config_path).ok().is_some_and(|doc| {
        doc.get("plugins")
            .and_then(Item::as_table)
            .and_then(|t| t.get(PLUGIN_ID))
            .and_then(Item::as_table)
            .and_then(|t| t.get("enabled"))
            .and_then(Item::as_bool)
            == Some(true)
    })
}

/// What `~/.codex/config.toml`'s `[marketplaces.rtok]` says (T140's counterpart of T139's own
/// `MarketplaceState` for Claude). Empirically verified against codex-cli 0.155.1: `codex
/// plugin marketplace add owner/repo` records `source_type = "git"` and `source =
/// "https://github.com/<owner>/<repo>.git"`; re-adding the identical source is a no-op, but a
/// `"rtok"` marketplace already pointing elsewhere errors "already added from a different
/// source" instead of re-pointing it, so a stale entry (a local-path marketplace from before
/// this flow, or a different checkout) must be removed before it is re-added from GitHub.
#[derive(PartialEq, Eq)]
enum MarketplaceState {
    /// No `"rtok"` entry at all.
    Absent,
    /// Points at `MARKETPLACE_GIT_SOURCE` already.
    Github,
    /// A `"rtok"` entry exists but not with that source.
    Stale,
}

fn marketplace_state(cfg: &Config) -> MarketplaceState {
    let Ok(doc) = load(&cfg.setup.codex.config_path) else {
        return MarketplaceState::Absent;
    };
    let Some(entry) = doc
        .get("marketplaces")
        .and_then(Item::as_table)
        .and_then(|t| t.get("rtok"))
        .and_then(Item::as_table)
    else {
        return MarketplaceState::Absent;
    };
    let is_github = entry.get("source_type").and_then(Item::as_str) == Some("git")
        && entry.get("source").and_then(Item::as_str) == Some(MARKETPLACE_GIT_SOURCE);
    if is_github {
        MarketplaceState::Github
    } else {
        MarketplaceState::Stale
    }
}

/// Offer, add, or remove rtok's Codex plugin through the real `codex plugin` commands
/// (T140, the Codex counterpart of T139's Claude flow), from the GitHub marketplace
/// `listepo/rtok`. Added by default — no flag needed — once `codex` is on PATH and the
/// plugin is not already enabled; already enabled from the GitHub marketplace (or already
/// removed) is a no-op. `marketplace add` is skipped once Codex already knows the *GitHub*
/// marketplace; a `"rtok"` marketplace known under any other source is re-pointed —
/// `marketplace remove` then `add` — instead of erroring or installing from the stale source
/// forever. A failing or missing `codex` keeps the offer open instead of failing the
/// install: the config.toml/hooks.json surfaces still go in (`apply`).
fn plugin(cfg: &Config, remove: bool) -> Result<String> {
    let a = apply(cfg);
    let installed = plugin_installed(cfg);
    let state = marketplace_state(cfg);
    if remove {
        if !installed {
            return Ok(NO_CHANGES.into());
        }
    } else if installed && state != MarketplaceState::Stale {
        return Ok(NO_CHANGES.into());
    }
    let steps: Vec<&[&str]> = if remove {
        vec![
            &["plugin", "remove", PLUGIN_ID],
            &["plugin", "marketplace", "remove", "rtok"],
        ]
    } else {
        let mut v: Vec<&[&str]> = Vec::new();
        if state == MarketplaceState::Stale {
            if installed {
                v.push(&["plugin", "remove", PLUGIN_ID]);
            }
            v.push(&["plugin", "marketplace", "remove", "rtok"]);
        }
        if state != MarketplaceState::Github {
            v.push(&["plugin", "marketplace", "add", MARKETPLACE_REPO]);
        }
        v.push(&["plugin", "add", PLUGIN_ID]);
        v
    };
    let shown = steps
        .iter()
        .map(|s| format!("codex {}", s.join(" ")))
        .collect::<Vec<_>>()
        .join(" && ");
    if a.dry_run {
        return Ok(if remove {
            format!("- plugin {PLUGIN_ID} ({shown})")
        } else {
            format!("offer {PLUGIN_SRC} → {shown} {KETCH_INSTALL}")
        });
    }
    if !remove && super::find_on_path("codex").is_none() {
        return Ok(format!(
            "offer {PLUGIN_SRC} → {shown} (codex failed: codex not found on PATH) {KETCH_INSTALL}"
        ));
    }
    for step in steps {
        if let Err(e) = codex_cli(cfg, step) {
            return Ok(format!("offer {PLUGIN_SRC} → {shown} (codex failed: {e})"));
        }
    }
    Ok(if remove {
        format!("- plugin {PLUGIN_ID}")
    } else {
        format!("+ plugin {PLUGIN_SRC} → {PLUGIN_ID}")
    })
}

/// Codex CLI: `[mcp_servers.rtok]` and, with `--proxy`, `[model_providers.rtok]`.
pub struct Codex;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "Codex",
    bins: &["codex"],
    apps: &[],
}];

impl Agent for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "mcp" => Support::Yes,
            "proxy" => Support::Flag("--proxy"),
            "hooks" => Support::Yes,
            // `plugin`: `plugins/codex` through `codex plugin marketplace add`, from the
            // GitHub marketplace `listepo/rtok`, enabled by default once `codex` is on PATH
            // and not already enabled (T140).
            "plugin" => Support::Yes,
            _ => Support::No("Codex has no such module"),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<std::path::PathBuf> {
        vec![cfg.setup.codex.config_path.clone(), hooks_path(cfg)]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let s = super::read(&cfg.setup.codex.config_path);
        // The enabled plugin serves the hooks and the MCP itself (D21).
        let plugin = plugin_installed(cfg);
        let mut out = Vec::new();
        if s.contains("[mcp_servers.rtok]") || plugin {
            out.push("mcp");
        }
        if s.contains("[model_providers.rtok]") {
            out.push("proxy");
        }
        if super::read(&hooks_path(cfg)).contains(" hook PreCompact") || plugin {
            out.push("hooks");
        }
        if plugin {
            out.push("plugin");
        }
        out
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        // Offer first: once the plugin is enabled it is the only call path (D21 singleton),
        // so the config.toml/hooks.json copies are stripped, not added — judged by Codex's
        // own record, so a dry run or a declined offer still gets the file-based install.
        let mut lines = vec![plugin(cfg, remove)?];
        if remove || plugin_installed(cfg) {
            lines.push(run(cfg, true)?);
            lines.push(run_hooks(cfg, true)?);
        } else {
            lines.push(run(cfg, false)?);
            lines.push(run_hooks(cfg, false)?);
        }
        // On the way out the provider block goes whether or not `--proxy` asked for it.
        if remove || cfg.setup.proxy {
            lines.push(register_proxy(cfg, remove)?);
        }
        lines.push(super::skill::sync("codex", cfg, remove)?);
        Ok(lines)
    }
}
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

/// Read Codex's config, or start from an empty document when the file is simply absent.
///
/// An unreadable file (non-UTF-8 byte, wrong permissions) is an error, not an empty
/// document: swallowing it made the installer overwrite a config it never read, leaving only
/// the `.bak-<ts>` as a way back.
fn load(path: &Path) -> Result<DocumentMut> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    raw.parse::<DocumentMut>()
        .with_context(|| path.display().to_string())
}

fn persist(cfg: &Config, path: &Path, doc: &DocumentMut, report: &str) -> Result<()> {
    rtok_agent_sdk::write(&apply(cfg), path, &doc.to_string(), report)
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
        return Ok(NO_CHANGES.into());
    }
    let cmd = super::rtok_command();
    let mut entry = Table::new();
    entry["command"] = value(cmd.as_str());
    let mut args = Array::new();
    args.push("mcp");
    entry["args"] = value(args);
    servers.insert(NAME, Item::Table(entry));
    Ok(format!(
        "+ [mcp_servers.rtok]\ncommand = \"{cmd}\"\nargs = [\"mcp\"]"
    ))
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
        NO_CHANGES.into()
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
    t.get("command")
        .and_then(Item::as_str)
        .is_some_and(super::is_rtok_bin)
        && args == ["mcp"]
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
        return Ok(NO_CHANGES.into());
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
        NO_CHANGES.into()
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
        assert!(out.starts_with("+ [mcp_servers.rtok]"), "{out}");
        assert!(out.contains("args = [\"mcp\"]"), "{out}");
        assert_eq!(out.matches("[mcp_servers.rtok]").count(), 1);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "model = \"o3\" # keep me\n"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// An unreadable config is not an empty one: the installer must fail rather than
    /// overwrite a file it could not read.
    #[test]
    fn an_unreadable_config_is_refused_not_overwritten() {
        let (c, path) = cfg("unreadable", false);
        let original = b"model = \"o3\"\n# \xff\xfe not utf-8\n";
        fs::write(&path, original).unwrap();
        let err = run(&c, false).unwrap_err();
        assert!(err.to_string().contains("config.toml"), "{err}");
        assert_eq!(fs::read(&path).unwrap(), original, "file untouched");
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
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.starts_with("# codex config\nmodel = \"o3\"\n"), "{raw}");
        assert!(
            raw.contains("[mcp_servers.other]\ncommand = \"x\"\n"),
            "{raw}"
        );
        assert!(raw.contains("[mcp_servers.rtok]"), "{raw}");
        assert!(raw.contains("args = [\"mcp\"]"), "{raw}");
        assert!(!raw.contains("\n[mcp_servers]\n"), "{raw}");
        let parsed: toml_edit::DocumentMut = raw.parse().unwrap();
        assert_eq!(
            parsed["mcp_servers"]["rtok"]["args"][0].as_str(),
            Some("mcp")
        );
        assert!(
            super::super::is_rtok_bin(
                parsed["mcp_servers"]["rtok"]["command"]
                    .as_str()
                    .unwrap_or("")
            ),
            "{raw}"
        );
        assert_eq!(run(&c, true).unwrap(), "- [mcp_servers.rtok]");
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        assert!(!fs::read_to_string(&path).unwrap().contains("rtok"));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_file_is_created_on_apply() {
        let (c, path) = cfg("new", false);
        run(&c, false).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("[mcp_servers.rtok]"), "{raw}");
        assert!(raw.contains("args = [\"mcp\"]"), "{raw}");
        assert!(
            super::super::is_rtok_bin(
                raw.parse::<toml_edit::DocumentMut>().unwrap()["mcp_servers"]["rtok"]["command"]
                    .as_str()
                    .unwrap_or("")
            ),
            "{raw}"
        );
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
        assert_eq!(register_proxy(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("model_provider = \"rtok\""), "{raw}");
        assert!(raw.contains("base_url"), "{raw}");
        assert!(raw.contains("# keep me"), "{raw}");
        assert_eq!(
            register_proxy(&c, true).unwrap(),
            "- [model_providers.rtok]"
        );
        assert_eq!(register_proxy(&c, true).unwrap(), NO_CHANGES);
        let gone = fs::read_to_string(&path).unwrap();
        assert!(!gone.contains("model_providers.rtok"), "{gone}");
        assert!(gone.contains("# keep me"), "{gone}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn compact_hooks_save_and_post_restores() {
        let (mut c, path) = cfg("compact", false);
        c.core.db_path = path.parent().unwrap().join("rtok.db");
        c.core.archive_dir = path.parent().unwrap().join("archive");
        let report = run_hooks(&c, false).unwrap();
        assert!(report.contains("PreCompact"), "{report}");
        assert!(report.contains("PostCompact"), "{report}");
        let raw = fs::read_to_string(hooks_path(&c)).unwrap();
        assert!(raw.contains("hook PreCompact"), "{raw}");
        assert!(raw.contains("hook PostCompact"), "{raw}");
        assert_eq!(Codex.installed(&c, Kind::Cli), ["hooks"]);

        let pre = serde_json::json!({
            "hook_event_name":"PreCompact",
            "session_id":"cdx",
            "transcript_path":"",
            "trigger":"auto"
        });
        let mut out = Vec::new();
        crate::hooks::run("PreCompact", pre.to_string().as_bytes(), &mut out, &c);
        assert_eq!(out, b"{}");
        assert!(
            crate::store::Store::open(&c.core.db_path)
                .unwrap()
                .latest_note("checkpoint:cdx")
                .unwrap()
                .is_some()
        );
        let post = serde_json::json!({
            "hook_event_name":"PostCompact",
            "session_id":"cdx",
            "trigger":"auto"
        });
        out.clear();
        crate::hooks::run("PostCompact", post.to_string().as_bytes(), &mut out, &c);
        let text = serde_json::from_slice::<serde_json::Value>(&out).unwrap()["hookSpecificOutput"]
            ["additionalContext"]
            .as_str()
            .unwrap_or("")
            .to_string();
        assert!(text.contains("checkpoint"), "{text}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    // --- T140: `plugin()` decision logic — enabled/known-marketplace/fresh, no real `codex` ---

    /// Already enabled from the GitHub marketplace → no-op, whether or not `--dry-run` is given.
    #[test]
    fn plugin_install_is_a_no_op_once_codex_already_enables_it() {
        let (c, path) = cfg("plugin-enabled", false);
        fs::write(
            &path,
            format!(
                "[marketplaces.rtok]\nsource_type = \"git\"\nsource = \"{MARKETPLACE_GIT_SOURCE}\"\n\n[plugins.\"rtok@rtok\"]\nenabled = true\n"
            ),
        )
        .unwrap();
        assert_eq!(plugin(&c, false).unwrap(), NO_CHANGES);
        let mut dry = c.clone();
        dry.setup.dry_run = true;
        assert_eq!(plugin(&dry, false).unwrap(), NO_CHANGES);
        assert!(plugin_installed(&c));
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// Not enabled and not removing it either → no-op (nothing to disable).
    #[test]
    fn plugin_remove_is_a_no_op_when_not_installed() {
        let (c, path) = cfg("plugin-remove-noop", false);
        assert_eq!(plugin(&c, true).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// A marketplace already pointed at the GitHub repo skips `marketplace add`: only the
    /// `plugin add` step is offered.
    #[test]
    fn plugin_dry_run_skips_marketplace_add_when_already_known() {
        let (c, path) = cfg("plugin-known-market", true);
        fs::write(
            &path,
            format!("[marketplaces.rtok]\nsource_type = \"git\"\nsource = \"{MARKETPLACE_GIT_SOURCE}\"\n"),
        )
        .unwrap();
        let report = plugin(&c, false).unwrap();
        assert!(report.contains("codex plugin add rtok@rtok"), "{report}");
        assert!(!report.contains("marketplace add"), "{report}");
        assert!(!report.contains("marketplace remove"), "{report}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// Nothing known yet: dry run previews both steps, from the GitHub marketplace, not a
    /// local path — and touches no file.
    #[test]
    fn plugin_dry_run_offers_both_steps_from_a_clean_install() {
        let (c, path) = cfg("plugin-fresh", true);
        let report = plugin(&c, false).unwrap();
        assert!(
            report.contains("codex plugin marketplace add listepo/rtok"),
            "{report}"
        );
        assert!(report.contains("codex plugin add rtok@rtok"), "{report}");
        assert!(report.starts_with("offer plugins/codex → "), "{report}");
        assert!(report.contains(KETCH_INSTALL), "{report}");
        assert!(!path.exists(), "{report}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// A `"rtok"` marketplace registered under a different source (e.g. a local-path
    /// checkout) is removed before it is re-added from GitHub — `marketplace add` on top of
    /// an existing entry errors instead of re-pointing it (verified against codex-cli
    /// 0.155.1: "already added from a different source").
    #[test]
    fn plugin_dry_run_repoints_a_stale_marketplace() {
        let (c, path) = cfg("plugin-stale-market", true);
        fs::write(
            &path,
            "[marketplaces.rtok]\nsource_type = \"local\"\nsource = \"/Users/x/rtok\"\n",
        )
        .unwrap();
        let report = plugin(&c, false).unwrap();
        assert!(
            report.contains(
                "codex plugin marketplace remove rtok && codex plugin marketplace add listepo/rtok && codex plugin add rtok@rtok"
            ),
            "{report}"
        );
        assert!(!report.contains("plugin remove"), "{report}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// The plugin already shows as enabled, but under a stale local marketplace: it is
    /// removed and re-added from GitHub instead of the usual already-installed no-op.
    #[test]
    fn plugin_dry_run_reinstalls_when_already_enabled_from_a_stale_marketplace() {
        let (c, path) = cfg("plugin-stale-installed", true);
        fs::write(
            &path,
            "[marketplaces.rtok]\nsource_type = \"local\"\nsource = \"/Users/x/rtok\"\n\n[plugins.\"rtok@rtok\"]\nenabled = true\n",
        )
        .unwrap();
        let report = plugin(&c, false).unwrap();
        assert_ne!(report, NO_CHANGES);
        assert!(
            report.contains(
                "codex plugin remove rtok@rtok && codex plugin marketplace remove rtok && codex plugin marketplace add listepo/rtok && codex plugin add rtok@rtok"
            ),
            "{report}"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
