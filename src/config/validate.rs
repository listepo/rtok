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
    let doc: DocumentMut = match text.parse() {
        Ok(d) => d,
        Err(e) => {
            return Ok(vec![format!("{}:{e}", path.display())]);
        }
    };
    let schema = FigValue::serialize(Config::default())
        .expect("Config serializes")
        .into_dict()
        .expect("Config is a table");
    let mut errors = Vec::new();
    check_table(path, &text, "", doc.as_table(), &schema, &mut errors);
    Ok(errors)
}

/// Edit `<home>/config.toml` at `key` (dotted), preserving comments. Creates the
/// reference file when it is missing.
pub fn set(home: &Path, key: &str, raw: &str) -> Result<PathBuf> {
    if key.is_empty() || key.split('.').any(|p| p.is_empty()) {
        bail!("empty key");
    }
    let path = Config::path_for(home);
    if !path.exists() {
        Config::init(home, false)?;
    }
    let text = std::fs::read_to_string(&path)?;
    let mut doc: DocumentMut = text.parse().with_context(|| path.display().to_string())?;
    assign(&mut doc, key, parse_value(raw))?;
    std::fs::write(&path, doc.to_string())?;
    Ok(path)
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
            "proxy.port" if !(1..=65535).contains(&n) => {
                errors.push(format!("{at}: {dotted} out of range (1–65535)"));
            }
            "plugins.archive.keep_turns" if n < 1 => {
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
    fn set_keeps_proxy_comment() {
        let home = tmp("set");
        Config::init(&home, false).unwrap();
        set(&home, "proxy.port", "8791").unwrap();
        let text = std::fs::read_to_string(Config::path_for(&home)).unwrap();
        assert!(
            text.contains("port            = 8791") || text.contains("port = 8791"),
            "{text}"
        );
        assert!(text.contains("# rtok proxy"), "{text}");
        let _ = std::fs::remove_dir_all(&home);
    }
}
