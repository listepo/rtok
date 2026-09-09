//! `rtok config validate` and `rtok config set` (plan T12.3, decision D14).
//!
//! Validate walks a TOML file against [`Config::default()`] and reports unknown keys,
//! wrong types, and out-of-range values with `file:line`. `set` writes the user file
//! through `toml_edit` so comments survive. Figment does not write files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use figment::value::{Dict, Value as FigValue};
use toml_edit::{DocumentMut, Item, Table, Value as TomlValue};

use super::Config;

/// Parse `path` and return human-readable errors (`file:line: …`). Empty = valid.
pub fn issues(path: &Path) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    Ok(issues_in(path, &text))
}

/// [`issues`] over text already in hand (`set` checks before it writes).
fn issues_in(path: &Path, text: &str) -> Vec<String> {
    let doc: DocumentMut = match text.parse() {
        Ok(d) => d,
        Err(e) => return vec![format!("{}:{e}", path.display())],
    };
    let schema = FigValue::serialize(Config::default())
        .expect("Config serializes")
        .into_dict()
        .expect("Config is a table");
    let mut errors = Vec::new();
    check_table(path, text, "", doc.as_table(), &schema, &mut errors);
    errors
}

/// Edit `<home>/config.toml` at `key` (dotted), preserving comments. Creates the
/// reference file when it is missing. Refuses a write that would fail [`issues`]
/// (unknown plugin id, `enabled = "yes"`, …) so the file never stops loading.
/// Returns the file and a `git diff` of the edit — empty when the value was already there.
/// `dry_run` renders that diff and writes nothing; the value is validated either way, so a
/// preview refuses exactly what the real run would refuse.
pub fn set(home: &Path, key: &str, raw: &str, dry_run: bool) -> Result<(PathBuf, String)> {
    if key.is_empty() || key.split('.').any(|p| p.is_empty()) {
        bail!("empty key");
    }
    let path = Config::path_for(home);
    if !path.exists() {
        if dry_run {
            bail!("no config file yet; run `rtok config init` first");
        }
        Config::init(home, false)?;
    }
    let before = std::fs::read_to_string(&path)?;
    let mut doc: DocumentMut = before.parse().with_context(|| path.display().to_string())?;
    assign(&mut doc, key, parse_value(raw))?;
    let after = doc.to_string();
    let errs = issues_in(&path, &after);
    if !errs.is_empty() {
        bail!("{}", errs.join("\n"));
    }
    let diff = crate::render::file_diff(&path, &before, &after);
    if !dry_run {
        std::fs::write(&path, after)?;
    }
    Ok((path, diff))
}

fn parse_value(raw: &str) -> TomlValue {
    raw.parse()
        .unwrap_or_else(|_| TomlValue::from(raw.to_string()))
}

fn assign(doc: &mut DocumentMut, key: &str, value: TomlValue) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    let item = toml_edit::value(value);
    match parts.as_slice() {
        [a] => doc[a] = item,
        [a, b] => doc[a][b] = item,
        [a, b, c] => doc[a][b][c] = item,
        [a, b, c, d] => doc[a][b][c][d] = item,
        _ => bail!("key too nested: {key}"),
    }
    Ok(())
}

fn is_open(dotted: &str) -> bool {
    dotted == "bench.configs"
}

fn line_of(src: &str, span: Option<std::ops::Range<usize>>) -> usize {
    let off = span.map(|s| s.start).unwrap_or(0).min(src.len());
    src[..off].bytes().filter(|&b| b == b'\n').count() + 1
}

fn loc(path: &Path, src: &str, item: &Item, dotted: &str) -> String {
    let leaf = dotted.rsplit('.').next().unwrap_or(dotted);
    let mut line = line_of(src, item.span());
    for (i, raw) in src.lines().enumerate() {
        let t = raw.trim_start();
        if t.strip_prefix(leaf)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
        {
            line = i + 1;
            break;
        }
    }
    format!("{}:{}", path.display(), line)
}

fn check_table(
    path: &Path,
    src: &str,
    prefix: &str,
    table: &Table,
    schema: &Dict,
    errors: &mut Vec<String>,
) {
    for (k, item) in table.iter() {
        let dotted = if prefix.is_empty() {
            k.to_string()
        } else {
            format!("{prefix}.{k}")
        };
        if is_open(prefix) {
            continue;
        }
        if is_open(&dotted) {
            continue;
        }
        match schema.get(k) {
            None => errors.push(format!(
                "{}: unknown key: {dotted}",
                loc(path, src, item, &dotted)
            )),
            Some(FigValue::Dict(_, nested)) => match item.as_table() {
                Some(t) => check_table(path, src, &dotted, t, nested, errors),
                None => errors.push(format!(
                    "{}: {dotted}: expected table",
                    loc(path, src, item, &dotted)
                )),
            },
            Some(expected) => check_leaf(path, src, dotted.as_str(), item, expected, errors),
        }
    }
}

fn check_leaf(
    path: &Path,
    src: &str,
    dotted: &str,
    item: &Item,
    expected: &FigValue,
    errors: &mut Vec<String>,
) {
    let at = loc(path, src, item, dotted);
    match expected {
        FigValue::String(..) => {
            if item.as_str().is_none() {
                errors.push(format!("{at}: {dotted}: expected string"));
                return;
            }
        }
        FigValue::Bool(..) => {
            if item.as_bool().is_none() {
                errors.push(format!("{at}: {dotted}: expected bool"));
                return;
            }
        }
        FigValue::Num(..) => {
            if item.as_integer().is_none() && item.as_float().is_none() {
                errors.push(format!("{at}: {dotted}: expected number"));
                return;
            }
        }
        FigValue::Array(..) if item.as_array().is_none() => {
            errors.push(format!("{at}: {dotted}: expected array"));
            return;
        }
        FigValue::Array(..) => {}
        _ => {}
    }
    if let Some(n) = item.as_integer() {
        match dotted {
            "proxy.port" | "web.port" if !(1..=65535).contains(&n) => {
                errors.push(format!("{at}: {dotted} out of range (1–65535)"));
            }
            "plugins.archive.keep_turns" if n < 1 => {
                errors.push(format!("{at}: {dotted} must be ≥ 1"));
            }
            "tui.tick_secs" if n < 1 => {
                errors.push(format!("{at}: {dotted} must be ≥ 1"));
            }
            "plugins.inject.budget_tokens" if n < 0 => {
                errors.push(format!("{at}: {dotted} must be ≥ 0"));
            }
            _ => {}
        }
    }
    if let Some(s) = item.as_str() {
        match dotted {
            "proxy.mode" if !matches!(s, "passthrough" | "compress") => {
                errors.push(format!("{at}: {dotted} must be passthrough or compress"));
            }
            "plugins.read.default_mode"
                if !matches!(s, "full" | "lines" | "map" | "signatures") =>
            {
                errors.push(format!(
                    "{at}: {dotted} must be full, lines, map, or signatures"
                ));
            }
            "plugins.graph.watch" if !matches!(s, "off" | "notify" | "watchman") => {
                errors.push(format!("{at}: {dotted} must be off, notify, or watchman"));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-val-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn port_70000_names_the_line() {
        let dir = tmp("port");
        let path = dir.join("bad.toml");
        std::fs::write(&path, "[proxy]\nport = 70000\n").unwrap();
        let errs = issues(&path).unwrap();
        assert!(
            errs.iter()
                .any(|e| e.contains(":2:") && e.contains("proxy.port")),
            "{errs:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plugin_enabled_is_validated() {
        let dir = tmp("plug");
        let path = dir.join("bad.toml");
        std::fs::write(
            &path,
            "[plugins.cmd]\nenabled = \"yes\"\n[plugins.nope]\nenabled = true\n",
        )
        .unwrap();
        let errs = issues(&path).unwrap();
        assert!(
            errs.iter()
                .any(|e| e.contains("plugins.cmd.enabled") && e.contains("bool")),
            "{errs:?}"
        );
        assert!(
            errs.iter().any(|e| e.contains("unknown key: plugins.nope")),
            "{errs:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_refuses_a_value_the_loader_would_reject() {
        let home = tmp("setbad");
        Config::init(&home, false).unwrap();
        let before = std::fs::read_to_string(Config::path_for(&home)).unwrap();
        assert!(set(&home, "plugins.nope.enabled", "true", false).is_err());
        assert!(set(&home, "plugins.cmd.enabled", "yes", false).is_err());
        assert_eq!(
            std::fs::read_to_string(Config::path_for(&home)).unwrap(),
            before
        );
        set(&home, "plugins.cmd.enabled", "false", false).unwrap();
        let cfg = Config::load_from(&home).unwrap();
        assert!(!cfg.plugin_enabled("cmd", true));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn set_keeps_proxy_comment() {
        let home = tmp("set");
        Config::init(&home, false).unwrap();
        set(&home, "proxy.port", "8791", false).unwrap();
        let text = std::fs::read_to_string(Config::path_for(&home)).unwrap();
        assert!(
            text.contains("port            = 8791") || text.contains("port = 8791"),
            "{text}"
        );
        assert!(text.contains("# rtok proxy"), "{text}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
