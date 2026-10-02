// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Duplicate MCP entries (T331.4): one agent would start the same server twice. Entries are read
//! with the `rtok_mcp` readers over the surfaces `agents` already knows (`agents::mcp::surfaces`),
//! plus Claude Code's project `.mcp.json` and local `projects.<cwd>` scopes. Two entries are
//! copies when their normalized launch matches: the command resolved through `PATH` and symlinks,
//! the same args and env, `npx -y pkg@x` as the package `pkg`, a URL compared after lowercasing
//! scheme and host and dropping a trailing slash. The same package at two versions is a conflict,
//! not a duplicate. The same name in two scopes is resolved by the host (local over project over
//! user, research.md section 25): the overridden entry is reported as unused. A server a file
//! marks disabled is not running and not compared. Env values take part in the comparison and are
//! never printed. rtok's own entry waits for T332/T333 and is neither compared nor reported.
//! Every copy is fixable (T331.6): `--fix` removes the ones that are not kept or used.

use std::collections::BTreeMap;
use std::path::Path;

use rtok_mcp::config::{self, Fs as McpFs};
use rtok_mcp::spec::McpSpec;
use serde_json::Value;

use super::hooks::{Probes, Problem};
use super::probe::Fs;

/// Where an entry lives, which decides the copy to keep and which same-name entry the host uses.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Project,
    User,
    Local,
}

impl Scope {
    /// Keep order, best first: the same rule as for hooks (project, user, local).
    fn keep(self) -> u8 {
        self as u8
    }

    /// Claude Code resolves a same-name entry by precedence: local, project, user.
    fn wins(self) -> u8 {
        match self {
            Scope::Local => 2,
            Scope::Project => 1,
            Scope::User => 0,
        }
    }

    fn why(self) -> &'static str {
        match self {
            Scope::Project => "shared project file",
            Scope::User => "user file",
            Scope::Local => "local scope",
        }
    }
}

/// One table of servers: the host's agent, its effective set, and the reader spec.
struct Source {
    agent: &'static str,
    set: String,
    spec: McpSpec,
    scope: Scope,
}

/// An enabled server entry as found.
struct Srv {
    agent: &'static str,
    set: String,
    source: String,
    path: String,
    name: String,
    /// What is printed: the command and args, or the URL. Never env.
    shown: String,
    /// The normalized launch, env included, version left out.
    key: String,
    version: Option<String>,
    scope: Scope,
    spec: McpSpec,
}

/// Where an entry sits in its file, for `--fix` to remove it (T331.6): the table and the name.
pub(super) struct Loc {
    pub source: String,
    pub path: String,
    pub spec: McpSpec,
    pub name: String,
}

/// The location of every entry the check reads; a finding's `source` and `path` find it.
pub(super) fn locations(cfg: &crate::config::Config, p: &Probes) -> Vec<Loc> {
    entries(cfg, p)
        .into_iter()
        .map(|s| Loc {
            source: s.source,
            path: s.path,
            spec: s.spec,
            name: s.name,
        })
        .collect()
}

/// The `rtok_mcp` reader over the injected [`Fs`], so mocks serve it too.
struct Reader<'a>(&'a dyn Fs);

impl McpFs for Reader<'_> {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        self.0.read(path).ok().map(String::into_bytes)
    }
    fn write(&mut self, _: &Path, _: Vec<u8>) -> anyhow::Result<()> {
        anyhow::bail!("the doctor check never writes")
    }
}

fn sources(cfg: &crate::config::Config, p: &Probes) -> Vec<Source> {
    let mut out: Vec<Source> = Vec::new();
    let cwd = p.env.cwd().filter(|d| !d.as_os_str().is_empty());
    for agent in crate::agents::HOSTS
        .iter()
        .filter_map(|id| crate::agents::host(id))
    {
        for v in agent.variants() {
            for s in crate::agents::mcp::surfaces(agent, cfg, v.kind) {
                let set = format!("{}:{}", agent.id(), s.label);
                let mut add = |spec: McpSpec, scope| {
                    let seen = |o: &Source| {
                        o.set == set
                            && o.spec.config_path == spec.config_path
                            && o.spec.key_path == spec.key_path
                    };
                    if !out.iter().any(seen) {
                        out.push(Source {
                            agent: agent.id(),
                            set: set.clone(),
                            spec,
                            scope,
                        });
                    }
                };
                if let (true, Some(cwd)) = (agent.id() == "claude" && s.label == "cli", &cwd) {
                    let project = McpSpec {
                        config_path: cwd.join(".mcp.json"),
                        key_path: vec!["mcpServers".into()],
                        ..s.spec.clone()
                    };
                    let local = McpSpec {
                        key_path: ["projects", &cwd.display().to_string(), "mcpServers"]
                            .map(String::from)
                            .into(),
                        ..s.spec.clone()
                    };
                    add(project, Scope::Project);
                    add(local, Scope::Local);
                }
                add(s.spec, Scope::User);
            }
        }
    }
    out
}

/// A server entry as `(shown, launch key, version)`; `None` for what is not compared.
fn launch(entry: &Value, p: &Probes) -> Option<(String, String, Option<String>)> {
    let get = |k: &str| entry.get(k).and_then(Value::as_str);
    let disabled = entry.get("disabled") == Some(&Value::Bool(true))
        || entry.get("enabled") == Some(&Value::Bool(false));
    if disabled || rtok_agent_sdk::runs_bin(entry, crate::agents::is_rtok_bin) {
        return None;
    }
    let env: BTreeMap<&str, &Value> = entry
        .get("env")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, v)| (k.as_str(), v))
        .collect();
    if let Some(url) = ["url", "serverUrl", "httpUrl"].iter().find_map(|k| get(k)) {
        let norm = url::Url::parse(url).map_or_else(
            |_| url.to_lowercase(),
            |u| u.as_str().trim_end_matches('/').to_string(),
        );
        let key = format!("url {norm} {} {env:?}", get("type").unwrap_or_default());
        return Some((url.to_string(), key, None));
    }
    // opencode and kilo spell the whole argv as one `command` array.
    let mut argv: Vec<String> = match entry.get("command")? {
        Value::String(c) => vec![c.clone()],
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        _ => return None,
    };
    argv.extend(
        entry
            .get("args")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from),
    );
    let shown = argv.join(" ");
    let (program, rest) = argv.split_first()?;
    let base = Path::new(program)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if ["npx", "bunx", "pnpx", "uvx"].contains(&base.as_str()) {
        let mut rest = rest.iter().skip_while(|a| a.starts_with('-'));
        if let Some(spec) = rest.next() {
            let at = spec.rfind('@').filter(|i| *i > 0);
            let (pkg, version) = match at {
                Some(i) => (&spec[..i], Some(spec[i + 1..].to_string())),
                None => (spec.as_str(), None),
            };
            let tail: Vec<&String> = rest.collect();
            return Some((shown, format!("{base} {pkg} {tail:?} {env:?}"), version));
        }
    }
    let path = if program.contains(['/', '\\']) {
        program.into()
    } else {
        p.which.find(program).unwrap_or_else(|| program.into())
    };
    let program = p.fs.canonical(&path).display().to_string();
    Some((shown, format!("{program} {rest:?} {env:?}"), None))
}

fn entries(cfg: &crate::config::Config, p: &Probes) -> Vec<Srv> {
    let mut out = Vec::new();
    for s in sources(cfg, p) {
        // A file that does not parse is `agents info`'s to report, not a duplicate check's.
        let Ok(Some(servers)) = config::read_servers(&Reader(p.fs), &s.spec) else {
            continue;
        };
        for (name, entry) in &servers {
            if let Some((shown, key, version)) = launch(entry, p) {
                out.push(Srv {
                    agent: s.agent,
                    set: s.set.clone(),
                    source: s.spec.config_path.display().to_string(),
                    path: format!("{}.{name}", s.spec.key_path.join(".")),
                    name: name.clone(),
                    shown,
                    key,
                    version,
                    scope: s.scope,
                    spec: s.spec.clone(),
                });
            }
        }
    }
    out
}

fn problem(s: &Srv, kind: &'static str, group: u32, keep: bool, detail: String) -> Problem {
    Problem {
        kind,
        agent: s.agent,
        source: s.source.clone(),
        path: s.path.clone(),
        event: String::new(),
        matcher: None,
        command: s.shown.clone(),
        detail,
        fixable: kind == "duplicate-mcp",
        group: Some(group),
        keep,
    }
}

/// Every group of copies, shadowed entries and version conflicts as one finding per entry.
pub fn check(cfg: &crate::config::Config, p: &Probes) -> Vec<Problem> {
    let all = entries(cfg, p);
    let mut out = Vec::new();
    let mut group = 0u32;
    let mut next = || {
        group += 1;
        group - 1
    };
    // The host uses one entry per name; the others never start.
    let mut by_name: BTreeMap<(&str, &str), Vec<&Srv>> = BTreeMap::new();
    for s in &all {
        by_name.entry((&s.set, &s.name)).or_default().push(s);
    }
    let mut live: Vec<&Srv> = Vec::new();
    for copies in by_name.values() {
        let used = copies
            .iter()
            .max_by_key(|s| s.scope.wins())
            .copied()
            .unwrap_or(copies[0]);
        live.push(used);
        if copies.len() > 1 {
            let g = next();
            for s in copies {
                let detail = if std::ptr::eq(*s, used) {
                    format!(
                        "`{}` is defined {} times; the host uses this one",
                        s.name,
                        copies.len()
                    )
                } else {
                    format!(
                        "`{}` is defined {} times; unused, overridden by another scope",
                        s.name,
                        copies.len()
                    )
                };
                out.push(problem(
                    s,
                    "duplicate-mcp",
                    g,
                    std::ptr::eq(*s, used),
                    detail,
                ));
            }
        }
    }
    let mut same: BTreeMap<(&str, &str, &Option<String>), Vec<&Srv>> = BTreeMap::new();
    let mut versions: BTreeMap<(&str, &str), Vec<&Srv>> = BTreeMap::new();
    for s in &live {
        same.entry((&s.set, &s.key, &s.version))
            .or_default()
            .push(s);
        versions.entry((&s.set, &s.key)).or_default().push(s);
    }
    for copies in same.values().filter(|c| c.len() > 1) {
        let g = next();
        // The first of equals is the first in load order.
        let keep = copies
            .iter()
            .min_by_key(|s| s.scope.keep())
            .copied()
            .unwrap_or(copies[0]);
        for s in copies {
            let me = std::ptr::eq(*s, keep);
            let detail = if me {
                format!(
                    "runs {} times; keep this copy ({})",
                    copies.len(),
                    s.scope.why()
                )
            } else {
                format!("runs {} times; an extra copy", copies.len())
            };
            out.push(problem(s, "duplicate-mcp", g, me, detail));
        }
    }
    for copies in versions.values() {
        let mut v: Vec<&Option<String>> = copies.iter().map(|s| &s.version).collect();
        v.sort();
        v.dedup();
        if v.len() > 1 {
            let g = next();
            for s in copies {
                let at = s.version.as_deref().unwrap_or("unpinned");
                let detail = format!(
                    "same package at {} versions, not duplicates; this one is {at}",
                    v.len()
                );
                out.push(problem(s, "conflicting-mcp", g, false, detail));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::doctor::probe::{Env, PathKind, Which};
    use std::io;
    use std::path::PathBuf;

    #[derive(Default)]
    struct Mock {
        files: BTreeMap<PathBuf, String>,
        links: BTreeMap<PathBuf, PathBuf>,
        path: BTreeMap<String, PathBuf>,
    }

    impl Fs for Mock {
        fn canonical(&self, path: &Path) -> PathBuf {
            self.links.get(path).cloned().unwrap_or_else(|| path.into())
        }
        fn read(&self, path: &Path) -> io::Result<String> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
        fn kind(&self, _: &Path) -> PathKind {
            PathKind::Missing
        }
    }
    impl Env for Mock {
        fn var(&self, _: &str) -> Option<String> {
            None
        }
        fn home(&self) -> Option<PathBuf> {
            Some("/h".into())
        }
        fn cwd(&self) -> Option<PathBuf> {
            Some("/proj".into())
        }
    }
    impl Which for Mock {
        fn find(&self, program: &str) -> Option<PathBuf> {
            self.path.get(program).cloned()
        }
    }

    fn cfg() -> Config {
        let mut c = Config::default();
        c.doctor.claude_json = "/h/.claude.json".into();
        c.setup.codex.config_path = "/h/.codex/config.toml".into();
        c
    }

    fn run(m: &Mock) -> Vec<Problem> {
        check(
            &cfg(),
            &Probes {
                fs: m,
                env: m,
                which: m,
            },
        )
    }

    fn user(m: &mut Mock, servers: &str) {
        m.files.insert(
            "/h/.claude.json".into(),
            format!(r#"{{"mcpServers": {{{servers}}}}}"#),
        );
    }

    /// `(kind, name, keep)` of every finding, for the groups' shape.
    fn shape(found: &[Problem]) -> Vec<(&'static str, String, bool)> {
        found
            .iter()
            .map(|p| {
                (
                    p.kind,
                    p.path.rsplit('.').next().unwrap_or_default().to_string(),
                    p.keep,
                )
            })
            .collect()
    }

    #[cfg(unix)] // POSIX paths
    #[test]
    fn two_names_with_one_launch_are_a_group_and_env_values_never_appear() {
        let mut m = Mock::default();
        user(
            &mut m,
            r#""a": {"command": "/bin/tool", "args": ["--x"], "env": {"TOKEN": "s3cret"}},
               "b": {"command": "/bin/tool", "args": ["--x"], "env": {"TOKEN": "s3cret"}},
               "c": {"command": "/bin/tool", "args": ["--y"], "env": {"TOKEN": "s3cret"}},
               "d": {"command": "/bin/tool", "args": ["--x"], "env": {"TOKEN": "other"}}"#,
        );
        let found = run(&m);
        assert_eq!(
            shape(&found),
            [
                ("duplicate-mcp", "a".into(), true),
                ("duplicate-mcp", "b".into(), false)
            ]
        );
        assert_eq!(found[0].detail, "runs 2 times; keep this copy (user file)");
        assert_eq!(found[0].command, "/bin/tool --x");
        let all = format!("{found:?}{}", crate::doctor::dupes::render_mcp(&found));
        assert!(!all.contains("s3cret") && !all.contains("TOKEN"), "{all}");
    }

    #[cfg(unix)]
    #[test]
    fn path_and_symlink_resolution_make_a_bare_command_equal_its_binary() {
        let mut m = Mock::default();
        m.path.insert("tool".into(), "/usr/local/bin/tool".into());
        m.links
            .insert("/usr/local/bin/tool".into(), "/opt/tool/bin/tool".into());
        user(
            &mut m,
            r#""a": {"command": "tool"}, "b": {"command": "/opt/tool/bin/tool"}, "c": {"command": "other"}"#,
        );
        assert_eq!(run(&m).len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn npx_versions_conflict_and_equal_ones_duplicate() {
        let mut m = Mock::default();
        user(
            &mut m,
            r#""a": {"command": "npx", "args": ["-y", "pkg@1.0.0"]},
               "b": {"command": "npx", "args": ["--yes", "pkg@2.0.0"]},
               "s1": {"command": "npx", "args": ["-y", "@sc/x@3"]},
               "s2": {"command": "npx", "args": ["@sc/x@3"]}"#,
        );
        let found = run(&m);
        assert_eq!(
            shape(&found),
            [
                ("duplicate-mcp", "s1".into(), true),
                ("duplicate-mcp", "s2".into(), false),
                ("conflicting-mcp", "a".into(), false),
                ("conflicting-mcp", "b".into(), false),
            ]
        );
        assert!(found[2].detail.contains("2 versions") && found[2].detail.ends_with("1.0.0"));
    }

    #[cfg(unix)]
    #[test]
    fn urls_compare_after_lowercasing_host_and_dropping_a_trailing_slash() {
        let mut m = Mock::default();
        user(
            &mut m,
            r#""a": {"type": "http", "url": "HTTPS://Example.COM/mcp/"},
               "b": {"type": "http", "url": "https://example.com/mcp"},
               "c": {"type": "sse", "url": "https://example.com/mcp"},
               "d": {"type": "http", "url": "https://example.com/Mcp"}"#,
        );
        assert_eq!(shape(&run(&m)).len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn disabled_servers_and_rtoks_own_entry_are_not_compared() {
        let mut m = Mock::default();
        user(
            &mut m,
            r#""a": {"command": "/bin/t", "disabled": true}, "b": {"command": "/bin/t"},
               "c": {"command": "/bin/u", "enabled": false}, "d": {"command": "/bin/u"},
               "rtok": {"command": "rtok", "args": ["mcp"]},
               "rtok-mcp": {"type": "stdio", "command": "/opt/bin/rtok", "args": ["mcp"]}"#,
        );
        assert!(run(&m).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn the_same_name_in_two_scopes_is_shadowed_by_local_then_project_then_user() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.claude.json".into(),
            r#"{"mcpServers": {"x": {"command": "/bin/user"}},
                "projects": {"/proj": {"mcpServers": {"x": {"command": "/bin/local"}}}}}"#
                .into(),
        );
        m.files.insert(
            "/proj/.mcp.json".into(),
            r#"{"mcpServers": {"x": {"command": "/bin/project"}}}"#.into(),
        );
        let found = run(&m);
        let by: Vec<(&str, bool)> = found.iter().map(|p| (p.source.as_str(), p.keep)).collect();
        assert_eq!(
            by,
            [
                ("/proj/.mcp.json", false),
                ("/h/.claude.json", true),
                ("/h/.claude.json", false)
            ]
        );
        assert!(found[1].path.starts_with("projects./proj.mcpServers"));
        assert!(found[0].detail.contains("unused"), "{}", found[0].detail);
        assert!(found.iter().all(|p| p.fixable));
    }

    #[cfg(unix)]
    #[test]
    fn a_toml_host_and_an_unparsable_file_are_handled() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.codex/config.toml".into(),
            "[mcp_servers.a]\ncommand = \"/bin/t\"\nargs = [\"1\"]\n[mcp_servers.b]\ncommand = \"/bin/t\"\nargs = [\"1\"]\n".into(),
        );
        m.files.insert("/h/.claude.json".into(), "{ nope".into());
        let found = run(&m);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].agent, "codex");
    }

    #[test]
    fn the_text_says_none_found_or_lists_each_group_once() {
        assert_eq!(
            crate::doctor::dupes::render_mcp(&[]),
            "duplicate mcp servers none found\n"
        );
    }
}
