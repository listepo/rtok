//! Config layering + precedence (plan T12.2, decision D14).
//!
//! Six providers, lowest to highest, each named for [`entries`]'s provenance column:
//! `default` (`Config::default()`) < `user` (`~/.rtok/config.toml` or `--config`/`RTOK_CONFIG`)
//! < `project` (`<git root>/.rtok.toml`) < `dotenv` (`RTOK_*` lines in `.env` files, T12.5)
//! < `env` (`RTOK_*`) < `flag` (CLI `Option<T>` fields).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use figment::providers::{Env, Format, Serialized, Toml};
use figment::value::{Dict, Map as FMap, Num, Value};
use figment::{Figment, Metadata, Profile, Provider};

use super::Config;

/// Wraps a provider so it reports under a different [`Metadata::name`] — used to rename the
/// two `Toml` file providers to `user` and `project` for [`entries`]'s provenance column.
struct Named<P>(P, &'static str);

impl<P: Provider> Provider for Named<P> {
    fn metadata(&self) -> Metadata {
        let mut m = self.0.metadata();
        m.name = self.1.into();
        m
    }

    fn data(&self) -> Result<FMap<Profile, Dict>, figment::Error> {
        self.0.data()
    }

    fn profile(&self) -> Option<Profile> {
        self.0.profile()
    }
}

/// The `flag` layer: a `Dict` built by `main.rs` from only the `Some` clap `Option<T>` fields.
/// `figment::value::Dict` doesn't implement `Serialize` (only the reverse, `Value::serialize`),
/// so `Serialized::defaults` can't take it directly — this is a minimal `Provider` instead.
struct FlagsProvider(Dict);

impl Provider for FlagsProvider {
    fn metadata(&self) -> Metadata {
        Metadata::named("flag")
    }

    fn data(&self) -> Result<FMap<Profile, Dict>, figment::Error> {
        Ok(Profile::Default.collect(self.0.clone()))
    }
}

/// Walk up from `start` looking for a `.git` entry (no subprocess). `None` outside a repo.
fn git_root(start: &Path) -> Option<PathBuf> {
    find_up(start, ".git")
}

/// The nearest directory at or above `start` that contains `name`.
fn find_up(start: &Path, name: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(name).exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// `RTOK_*` pairs (prefix stripped, uppercased) from the nearest `.env` at or above `cwd`,
/// then `<home>/.env`; the first file to define a key wins. Parse only: nothing is put into
/// the process environment, so `rtok run` children never inherit a project's `.env`, and
/// non-`RTOK_` keys are ignored. A malformed file is one stderr line, not an error (fail open).
fn dotenv_pairs(home: &Path, cwd: Option<&Path>) -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = cwd
        .and_then(|c| find_up(c, ".env"))
        .map(|d| d.join(".env"))
        .into_iter()
        .collect();
    files.push(home.join(".env"));
    let mut out: Vec<(String, String)> = Vec::new();
    for path in files {
        let iter = match dotenvy::from_path_iter(&path) {
            Ok(it) => it,
            Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                eprintln!("rtok: {}: {e}", path.display());
                continue;
            }
        };
        for item in iter {
            match item {
                Ok((k, v)) => {
                    let Some(suffix) = k.strip_prefix("RTOK_") else {
                        continue;
                    };
                    let suffix = suffix.to_uppercase();
                    if !out.iter().any(|(s, _)| *s == suffix) {
                        out.push((suffix, v));
                    }
                }
                Err(e) => {
                    eprintln!("rtok: {}: {e}", path.display());
                    break;
                }
            }
        }
    }
    out
}

/// `RTOK_<SECTION>_<KEY...>` → `section.key...`, plus whether the default value is an array
/// (so the env provider knows to split on `,`). Built once from `Config::default()` so a
/// dotted key's canonical env name is always `key.to_uppercase().replace('.', "_")` — no
/// guessing which underscores in `openai_upstream` or `budget_tokens` are separators.
fn env_leaf_table() -> BTreeMap<String, (String, bool)> {
    let mut table = BTreeMap::new();
    let root = Value::serialize(Config::default()).expect("Config serializes");
    if let Some(dict) = root.into_dict() {
        walk(&dict, "", &mut table);
    }
    table
}

fn walk(dict: &Dict, prefix: &str, out: &mut BTreeMap<String, (String, bool)>) {
    for (k, v) in dict {
        let dotted = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            Value::Dict(_, d) => walk(d, &dotted, out),
            other => {
                let is_array = matches!(other, Value::Array(..));
                let canonical = dotted.to_uppercase().replace('.', "_");
                out.insert(canonical, (dotted, is_array));
            }
        }
    }
}

/// Legacy short env-var aliases kept for backward compatibility (predate the dotted scheme).
const LEGACY_ALIASES: &[(&str, &str)] = &[
    ("UPSTREAM", "proxy.upstream"),
    ("OPENAI_UPSTREAM", "proxy.openai_upstream"),
];

/// The `env` layer. `figment::providers::Env` alone can't be used as-is: it splits every `_`
/// into a nesting boundary, which mangles keys like `proxy.openai_upstream`. Instead this
/// looks up each `RTOK_*` var (and the legacy aliases) in [`env_leaf_table`], drops anything
/// not found (so `RTOK_HOME`/`RTOK_CONFIG` are ignored rather than becoming unknown keys under
/// `deny_unknown_fields`), and splits comma-separated values into an array for keys whose
/// default is an array.
struct RtokEnv {
    /// Provenance name: `env` (process) or `dotenv` (`.env` files) — same resolution, two layers.
    name: &'static str,
    table: BTreeMap<String, (String, bool)>,
    /// `RTOK_`-stripped, uppercased names. Empty in tests so ambient env cannot leak.
    vars: Vec<(String, String)>,
}

impl RtokEnv {
    fn from_process() -> Self {
        Self {
            name: "env",
            table: env_leaf_table(),
            vars: Env::prefixed("RTOK_")
                .iter()
                .map(|(k, v)| (k.as_str().to_uppercase(), v))
                .collect(),
        }
    }

    fn from_dotenv(home: &Path, cwd: Option<&Path>) -> Self {
        Self {
            name: "dotenv",
            table: env_leaf_table(),
            vars: dotenv_pairs(home, cwd),
        }
    }

    #[cfg(test)]
    fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self {
            name: "env",
            table: env_leaf_table(),
            vars: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    #[cfg(test)]
    fn from_dotenv_pairs(pairs: &[(&str, &str)]) -> Self {
        Self {
            name: "dotenv",
            ..Self::from_pairs(pairs)
        }
    }

    /// `RTOK_`-stripped, uppercased var name → `(dotted key, value)`, or `None` if unknown.
    fn resolve(&self, suffix: &str, raw: &str) -> Option<(String, Value)> {
        let (dotted, is_array) = LEGACY_ALIASES
            .iter()
            .find(|(alias, _)| *alias == suffix)
            .map(|(_, dotted)| (dotted.to_string(), false))
            .or_else(|| {
                self.table
                    .get(suffix)
                    .map(|(dotted, arr)| (dotted.clone(), *arr))
            })?;

        let value = if is_array {
            Value::from(
                raw.split(',')
                    .map(|s| Value::from(s.trim()))
                    .collect::<Vec<_>>(),
            )
        } else {
            raw.parse().expect("Value::from_str is infallible")
        };
        Some((dotted, value))
    }
}

/// Insert `value` at the dotted path `key` into `dict`, creating intermediate dicts and
/// descending into ones that already exist (so `RTOK_PROXY_PORT` and
/// `RTOK_PROXY_OPENAI_UPSTREAM` both land under one `proxy` dict rather than overwriting).
fn insert_dotted(dict: &mut Dict, key: &str, value: Value) {
    match key.split_once('.') {
        Some((head, rest)) => {
            let entry = dict
                .entry(head.to_string())
                .or_insert_with(|| Value::from(Dict::new()));
            if let Value::Dict(_, d) = entry {
                insert_dotted(d, rest, value);
            } else {
                let mut d = Dict::new();
                insert_dotted(&mut d, rest, value);
                *entry = Value::from(d);
            }
        }
        None => {
            dict.insert(key.to_string(), value);
        }
    }
}

impl Provider for RtokEnv {
    fn metadata(&self) -> Metadata {
        Metadata::named(self.name)
    }

    fn data(&self) -> Result<FMap<Profile, Dict>, figment::Error> {
        let mut dict = Dict::new();
        for (suffix, raw) in &self.vars {
            if let Some((dotted, value)) = self.resolve(suffix, raw) {
                insert_dotted(&mut dict, &dotted, value);
            }
        }
        Ok(Profile::Default.collect(dict))
    }
}

/// Build the layered [`Figment`] (does not extract/validate — see [`load`]). `config_file`
/// picks the user file (else `RTOK_CONFIG` env or `<home>/config.toml`); `flags` is the `flag`
/// layer, built by the caller from only the `Some` CLI options.
pub fn figment(home: &Path, config_file: Option<&Path>, flags: Option<Dict>) -> Figment {
    let cwd = std::env::current_dir().ok();
    assemble(
        home,
        config_file,
        flags,
        RtokEnv::from_process(),
        cwd.as_deref(),
        RtokEnv::from_dotenv(home, cwd.as_deref()),
    )
}

fn assemble(
    home: &Path,
    config_file: Option<&Path>,
    flags: Option<Dict>,
    env: RtokEnv,
    cwd: Option<&Path>,
    dotenv: RtokEnv,
) -> Figment {
    let user_path = config_file
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("RTOK_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| Config::path_for(home));

    let mut fig = Figment::from(Named(Serialized::defaults(Config::default()), "default"))
        .merge(Named(Toml::file(&user_path), "user"));

    if let Some(root) = cwd.and_then(git_root) {
        fig = fig.merge(Named(Toml::file(root.join(".rtok.toml")), "project"));
    }

    fig = fig.merge(dotenv).merge(env);

    if let Some(flags) = flags {
        fig = fig.merge(FlagsProvider(flags));
    }

    fig
}

/// Clap `Option<T>` overlay for `rtok proxy` (`flag` layer). Only `Some` / `--dry-run`.
pub fn proxy_flags(
    port: Option<u16>,
    dry_run: bool,
    upstream: Option<String>,
    mode: Option<String>,
) -> Option<Dict> {
    let mut proxy = Dict::new();
    if let Some(port) = port {
        proxy.insert("port".into(), Value::from(i64::from(port)));
    }
    if dry_run {
        proxy.insert("dry_run".into(), Value::from(true));
    }
    if let Some(upstream) = upstream {
        proxy.insert("upstream".into(), Value::from(upstream));
    }
    if let Some(mode) = mode {
        proxy.insert("mode".into(), Value::from(mode));
    }
    if proxy.is_empty() {
        return None;
    }
    let mut flags = Dict::new();
    flags.insert("proxy".into(), Value::from(proxy));
    Some(flags)
}

/// [`figment`], extracted and finished (legacy-key migration + `~` expansion).
pub fn load(home: &Path, config_file: Option<&Path>, flags: Option<Dict>) -> Result<Config> {
    let mut cfg: Config = figment(home, config_file, flags).extract()?;
    cfg.finish(home);
    Ok(cfg)
}

/// One row per effective key, sorted, for `config show`/`config get`: `(dotted key, value as
/// a display string, source layer name)`.
/// Dotted keys present in [`Config::default()`], for the T12.4 coverage walk.
pub fn leaf_keys() -> Vec<String> {
    let mut keys: Vec<String> = env_leaf_table()
        .values()
        .map(|(dotted, _)| dotted.clone())
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

pub fn entries(fig: &Figment) -> Vec<(String, String, String)> {
    let table = env_leaf_table();
    let mut keys: Vec<&String> = table.values().map(|(dotted, _)| dotted).collect();
    keys.sort();
    keys.dedup();

    keys.into_iter()
        .filter_map(|key| {
            let value = fig.find_value(key).ok()?;
            let source = fig
                .find_metadata(key)
                .map(|m| m.name.to_string())
                .unwrap_or_else(|| "default".to_string());
            Some((key.clone(), display(&value), source))
        })
        .collect()
}

/// `figment::value::Value` has no `Display` impl (only `Num`/`Bool`/etc. do internally) — this
/// renders the same shape `config show`/`get` need: TOML-ish scalars, `[a, b]` for arrays.
fn display(v: &Value) -> String {
    match v {
        Value::String(_, s) => s.clone(),
        Value::Char(_, c) => c.to_string(),
        Value::Bool(_, b) => b.to_string(),
        Value::Num(_, n) => match *n {
            Num::U8(v) => v.to_string(),
            Num::U16(v) => v.to_string(),
            Num::U32(v) => v.to_string(),
            Num::U64(v) => v.to_string(),
            Num::U128(v) => v.to_string(),
            Num::USize(v) => v.to_string(),
            Num::I8(v) => v.to_string(),
            Num::I16(v) => v.to_string(),
            Num::I32(v) => v.to_string(),
            Num::I64(v) => v.to_string(),
            Num::I128(v) => v.to_string(),
            Num::ISize(v) => v.to_string(),
            Num::F32(v) => v.to_string(),
            Num::F64(v) => v.to_string(),
        },
        Value::Empty(..) => String::new(),
        Value::Dict(_, d) => {
            let parts: Vec<String> = d
                .iter()
                .map(|(k, v)| format!("{k} = {}", display(v)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(_, a) => {
            let parts: Vec<String> = a.iter().map(display).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rtok-layers-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Isolated figment: no process env, no cwd (so no project file), user file at `home`.
    fn fig(home: &Path, env: &[(&str, &str)], flags: Option<Dict>) -> Figment {
        assemble(
            home,
            Some(&Config::path_for(home)),
            flags,
            RtokEnv::from_pairs(env),
            None,
            RtokEnv::from_dotenv_pairs(&[]),
        )
    }

    #[test]
    fn default_only_has_default_source() {
        let home = tmp("default-only");
        let rows = entries(&fig(&home, &[], None));
        let port = rows.iter().find(|(k, ..)| k == "proxy.port").unwrap();
        assert_eq!(port.1, "8790");
        assert_eq!(port.2, "default");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn user_file_overrides_default() {
        let home = tmp("user-file");
        std::fs::write(Config::path_for(&home), "[proxy]\nport = 9999\n").unwrap();
        let rows = entries(&fig(&home, &[], None));
        let port = rows.iter().find(|(k, ..)| k == "proxy.port").unwrap();
        assert_eq!(port.1, "9999");
        assert_eq!(port.2, "user");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn env_and_flags_and_legacy_and_arrays() {
        let home = tmp("env");

        let rows = entries(&fig(&home, &[("PROXY_PORT", "1234")], None));
        let port = rows.iter().find(|(k, ..)| k == "proxy.port").unwrap();
        assert_eq!(port.1, "1234");
        assert_eq!(port.2, "env");

        let flags = proxy_flags(Some(4321), false, None, None);
        let rows = entries(&fig(&home, &[("PROXY_PORT", "1234")], flags));
        let port = rows.iter().find(|(k, ..)| k == "proxy.port").unwrap();
        assert_eq!(port.1, "4321");
        assert_eq!(port.2, "flag");

        let rows = entries(&fig(&home, &[("PROXY_OPENAI_UPSTREAM", "https://x")], None));
        let key = rows
            .iter()
            .find(|(k, ..)| k == "proxy.openai_upstream")
            .unwrap();
        assert_eq!(key.1, "https://x");
        assert_eq!(key.2, "env");

        let rows = entries(&fig(&home, &[("UPSTREAM", "https://legacy")], None));
        let key = rows.iter().find(|(k, ..)| k == "proxy.upstream").unwrap();
        assert_eq!(key.1, "https://legacy");
        assert_eq!(key.2, "env");

        let figment = assemble(
            &home,
            Some(&Config::path_for(&home)),
            None,
            RtokEnv::from_pairs(&[("PLUGINS_READ_ALLOW_PATHS", "/a,/b")]),
            None,
            RtokEnv::from_dotenv_pairs(&[]),
        );
        let cfg: Config = figment.extract().unwrap();
        assert_eq!(cfg.plugins.read.allow_paths.len(), 2);

        let figment = assemble(
            &home,
            Some(&Config::path_for(&home)),
            None,
            RtokEnv::from_pairs(&[("HOME", "/tmp/rtok-layers-home-probe")]),
            None,
            RtokEnv::from_dotenv_pairs(&[]),
        );
        assert!(figment.extract::<Config>().is_ok());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn dotenv_files_take_rtok_keys_project_first() {
        let home = tmp("dotenv-home");
        let proj = tmp("dotenv-proj");
        let sub = proj.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            home.join(".env"),
            "RTOK_PROXY_PORT=8801\nRTOK_PROXY_MODE=compress\nRTOK_T125_LEAK=1\n",
        )
        .unwrap();
        std::fs::write(
            proj.join(".env"),
            "# project\nRTOK_PROXY_PORT=8799\nDATABASE_URL=postgres://x\n",
        )
        .unwrap();
        let pairs = dotenv_pairs(&home, Some(&sub));
        assert_eq!(
            pairs,
            vec![
                ("PROXY_PORT".to_string(), "8799".to_string()),
                ("PROXY_MODE".to_string(), "compress".to_string()),
                ("T125_LEAK".to_string(), "1".to_string()),
            ]
        );
        assert!(std::env::var_os("RTOK_T125_LEAK").is_none(), "parse only");
        assert_eq!(dotenv_pairs(&home, None)[0].1, "8801");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn dotenv_layer_sits_between_project_and_env() {
        let home = tmp("dotenv-layer");
        let with = |env: &[(&str, &str)]| {
            let figment = assemble(
                &home,
                Some(&Config::path_for(&home)),
                None,
                RtokEnv::from_pairs(env),
                None,
                RtokEnv::from_dotenv_pairs(&[("PROXY_PORT", "8799")]),
            );
            let rows = entries(&figment);
            let port = rows.iter().find(|(k, ..)| k == "proxy.port").unwrap();
            (port.1.clone(), port.2.clone())
        };
        assert_eq!(with(&[]), ("8799".to_string(), "dotenv".to_string()));
        assert_eq!(
            with(&[("PROXY_PORT", "8800")]),
            ("8800".to_string(), "env".to_string())
        );
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn malformed_dotenv_is_skipped_not_fatal() {
        let home = tmp("dotenv-bad");
        std::fs::write(
            home.join(".env"),
            "RTOK_PROXY_PORT=8799\nthis line is not valid\nRTOK_PROXY_MODE=compress\n",
        )
        .unwrap();
        let pairs = dotenv_pairs(&home, None);
        assert_eq!(pairs.first().map(|p| p.0.as_str()), Some("PROXY_PORT"));
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn project_file_is_the_source() {
        let home = tmp("project-home");
        let repo = tmp("project-repo");
        std::fs::write(repo.join(".git"), "").unwrap();
        std::fs::write(
            repo.join(".rtok.toml"),
            "[plugins.read]\nallow_paths = [\"/proj\"]\n",
        )
        .unwrap();
        let figment = assemble(
            &home,
            Some(&Config::path_for(&home)),
            None,
            RtokEnv::from_pairs(&[]),
            Some(&repo),
            RtokEnv::from_dotenv_pairs(&[]),
        );
        let rows = entries(&figment);
        let key = rows
            .iter()
            .find(|(k, ..)| k == "plugins.read.allow_paths")
            .unwrap();
        assert_eq!(key.1, "[/proj]");
        assert_eq!(key.2, "project");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(&repo);
    }
}
