//! aider installer (`rtok agents install aider --proxy`, plan T48.7).
//!
//! aider is a terminal CLI with no MCP servers and no hook events, so the proxy is the
//! only rtok surface it can use: `openai-api-base` in `.aider.conf.yml` (home dir, git
//! root or cwd; setup writes the home file) points aider's OpenAI-compatible path at
//! `rtok proxy`. There is no `anthropic-api-base` in aider's options reference —
//! Anthropic models reach the same base URL with an `openai/` model prefix. The YAML is
//! edited line-wise so comments survive; no YAML crate (no new dependency).

use std::fs;

use anyhow::{Context, Result};
use rtok_agent_sdk::NO_CHANGES;

use super::{Agent, Kind, Mode, Support, Variant, apply};
use crate::config::Config;

/// The only YAML key setup writes: aider's OpenAI-compatible base URL.
const KEY: &str = "openai-api-base";

/// aider: proxy-only over `.aider.conf.yml`.
pub struct Aider;

static VARIANTS: [Variant; 1] = [Variant {
    kind: Kind::Cli,
    name: "aider",
    bins: &["aider"],
    apps: &[],
}];

impl Agent for Aider {
    fn id(&self) -> &'static str {
        "aider"
    }

    fn variants(&self) -> &'static [Variant] {
        &VARIANTS
    }

    fn readme(&self) -> &'static str {
        include_str!("README.md")
    }

    fn support(&self, _kind: Kind, module: &str) -> Support {
        match module {
            "proxy" => Support::Flag("--proxy"),
            "hooks" => Support::No("aider has no hook events; it reads .aider.conf.yml and .env"),
            "mcp" => Support::No("aider has no MCP support"),
            _ => Support::No("aider has no plugin directory to link"),
        }
    }

    fn files(&self, cfg: &Config, _kind: Kind) -> Vec<std::path::PathBuf> {
        vec![cfg.setup.aider.config_path.clone()]
    }

    fn installed(&self, cfg: &Config, _kind: Kind) -> Vec<&'static str> {
        let url = super::openai_proxy_url(cfg);
        if value_of(
            &super::read(&cfg.setup.aider.config_path),
            &url,
            cfg.proxy.port,
        )
        .is_some()
        {
            vec!["proxy"]
        } else {
            Vec::new()
        }
    }

    fn apply(&self, cfg: &Config, _kind: Kind, mode: Mode) -> Result<Vec<String>> {
        let remove = mode == Mode::Remove;
        if remove || cfg.setup.proxy {
            Ok(vec![run(cfg, remove)?])
        } else {
            Ok(vec![NO_CHANGES.into()])
        }
    }
}

/// Apply, dry-run, or remove `openai-api-base` in the YAML config.
///
/// An absent file is an empty document; an unreadable one is an error, never an
/// overwrite of a config that was not read.
pub fn run(cfg: &Config, remove: bool) -> Result<String> {
    let path = &cfg.setup.aider.config_path;
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| path.display().to_string()),
    };
    let url = super::openai_proxy_url(cfg);
    let (body, report) = if remove {
        strip_ours(&raw, &url, cfg.proxy.port)
    } else {
        insert_ours(&raw, &url)
    };
    rtok_agent_sdk::write(&apply(cfg), path, &body, &report)?;
    Ok(report)
}

/// An uncommented `openai-api-base: <value>` line as `(indent, value, comment)`.
/// `None` for comments, other keys, and lookalikes (`openai-api-base-foo`).
fn split_key(line: &str) -> Option<(&str, &str, Option<&str>)> {
    let indent = line.len() - line.trim_start().len();
    let rest = line[indent..].strip_prefix(KEY)?;
    let after = rest
        .strip_prefix([' ', '\t'])
        .map(|s| s.trim_start_matches([' ', '\t']))
        .unwrap_or(rest);
    let tail = after.strip_prefix(':')?;
    let (value, comment) = split_comment(tail);
    Some((&line[..indent], unquote(value.trim()), comment))
}

/// Split `value # comment` at a `#` that starts the remainder or follows whitespace.
/// A URL never carries a bare `#`, and the proxy URL never carries one at all.
fn split_comment(tail: &str) -> (&str, Option<&str>) {
    let bytes = tail.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return (tail[..i].trim_end(), Some(tail[i..].trim()));
        }
        i += 1;
    }
    (tail, None)
}

fn unquote(value: &str) -> &str {
    let b = value.as_bytes();
    if value.len() >= 2
        && ((b[0] == b'"' && b[value.len() - 1] == b'"')
            || (b[0] == b'\'' && b[value.len() - 1] == b'\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

/// True when `value` is the proxy URL setup wrote: exactly it, or a loopback URL on
/// our port (so `list` still reads back an install after `--port` moved it there).
fn is_ours_value(value: &str, url: &str, port: u16) -> bool {
    if value == url {
        return true;
    }
    let loopback = value.contains("127.0.0.1") || value.contains("localhost");
    loopback && port_at(value, port)
}

/// `:{port}` followed by a non-digit or the end, so `:8790` never matches `:87901`.
fn port_at(value: &str, port: u16) -> bool {
    let needle = format!(":{port}");
    value
        .split(&needle)
        .skip(1)
        .any(|rest| rest.chars().next().is_none_or(|c| !c.is_ascii_digit()))
}

/// The current `openai-api-base` value when it points at this proxy.
fn value_of(raw: &str, url: &str, port: u16) -> Option<String> {
    raw.lines()
        .filter_map(split_key)
        .map(|(_, v, _)| v.to_string())
        .find(|v| is_ours_value(v, url, port))
}

fn insert_ours(raw: &str, url: &str) -> (String, String) {
    let mut out = Vec::new();
    let mut changed = false;
    let mut old: Option<String> = None;
    for line in raw.lines() {
        match split_key(line) {
            Some((indent, value, comment)) if value != url => {
                old = Some(value.to_string());
                let tail = comment.map_or_else(String::new, |c| format!(" {c}"));
                out.push(format!("{indent}{KEY}: {url}{tail}"));
                changed = true;
            }
            _ => out.push(line.to_string()),
        }
    }
    if !changed {
        if raw.lines().filter_map(split_key).next().is_some() {
            return (raw.to_string(), NO_CHANGES.into());
        }
        let mut body = raw.to_string();
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&format!("{KEY}: {url}\n"));
        let report = format!("+ {KEY}: {url}\nrevert: remove {KEY}");
        return (body, report);
    }
    let mut body = out.join("\n");
    if raw.ends_with('\n') || raw.is_empty() {
        body.push('\n');
    }
    let revert = match old.as_deref() {
        Some(v) => format!("revert: set {KEY} to {v}"),
        None => format!("revert: remove {KEY}"),
    };
    (body, format!("+ {KEY}: {url}\n{revert}"))
}

/// Drop `openai-api-base` lines that point at this proxy; a foreign base URL stays.
fn strip_ours(raw: &str, url: &str, port: u16) -> (String, String) {
    let keep: Vec<&str> = raw
        .lines()
        .filter(|l| match split_key(l) {
            Some((_, value, _)) => !is_ours_value(value, url, port),
            None => true,
        })
        .collect();
    if keep.len() == raw.lines().count() {
        return (raw.to_string(), NO_CHANGES.into());
    }
    let mut body = keep.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    (body, format!("- {KEY}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn cfg(dir: &str, dry: bool) -> (Config, PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-aider-{dir}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".aider.conf.yml");
        let mut c = Config::default();
        c.setup.aider.config_path = path.clone();
        c.setup.dry_run = dry;
        c.setup.backup = false;
        (c, path)
    }

    #[test]
    fn dry_run_shows_one_key_and_creates_nothing() {
        let (c, path) = cfg("dry", true);
        fs::write(&path, "# mine\nmodel: openai/gpt-4o\n").unwrap();
        let out = run(&c, false).unwrap();
        assert!(out.contains("+ openai-api-base: http://"), "{out}");
        assert!(out.contains("8790/v1"), "{out}");
        assert!(out.contains("revert:"), "{out}");
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "# mine\nmodel: openai/gpt-4o\n"
        );
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn apply_is_idempotent_and_keeps_comments_and_foreign_keys() {
        let (c, path) = cfg("apply", false);
        fs::write(
            &path,
            "# aider config\nmodel: openai/gpt-4o\n# openai-api-base: https://old.example\n",
        )
        .unwrap();
        let first = run(&c, false).unwrap();
        assert!(first.contains("+ openai-api-base:"), "{first}");
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            raw.starts_with("# aider config\nmodel: openai/gpt-4o\n"),
            "{raw}"
        );
        assert!(
            raw.contains("# openai-api-base: https://old.example"),
            "commented line untouched: {raw}"
        );
        assert!(raw.contains("openai-api-base: http://"), "{raw}");
        assert_eq!(Aider.installed(&c, Kind::Cli), ["proxy"]);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn missing_file_is_created_on_apply() {
        let (c, path) = cfg("new", false);
        run(&c, false).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("openai-api-base: http://"), "{raw}");
        assert!(raw.contains("8790/v1"), "{raw}");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn quoted_value_and_trailing_comment_keep_their_shape() {
        let (c, path) = cfg("quoted", false);
        fs::write(&path, "openai-api-base: \"https://old.example\" # mine\n").unwrap();
        run(&c, false).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("openai-api-base: http://"), "{raw}");
        assert!(raw.ends_with("# mine\n"), "{raw}");
        assert_eq!(run(&c, false).unwrap(), NO_CHANGES);
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn remove_strips_only_ours_and_keeps_a_foreign_base_url() {
        let (c, path) = cfg("remove", false);
        fs::write(
            &path,
            "# mine\nopenai-api-base: https://foreign.example/v1\n",
        )
        .unwrap();
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("https://foreign.example/v1"),
            "foreign base URL stays"
        );
        run(&c, false).unwrap();
        assert_eq!(run(&c, true).unwrap(), "- openai-api-base");
        assert_eq!(run(&c, true).unwrap(), NO_CHANGES);
        let gone = fs::read_to_string(&path).unwrap();
        assert!(!gone.contains("openai-api-base:"), "{gone}");
        assert!(gone.contains("# mine"), "comments survive: {gone}");
        assert!(Aider.installed(&c, Kind::Cli).is_empty());
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    /// An unreadable config is not an empty one: refuse rather than overwrite it.
    #[test]
    fn an_unreadable_config_is_refused_not_overwritten() {
        let (c, path) = cfg("unreadable", false);
        let original = b"model: openai/gpt-4o\n# \xff\xfe not utf-8\n";
        fs::write(&path, original).unwrap();
        let err = run(&c, false).unwrap_err();
        assert!(err.to_string().contains(".aider.conf.yml"), "{err}");
        assert_eq!(fs::read(&path).unwrap(), original, "file untouched");
        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
