// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! Broken hooks (T331.1): a hook whose command leads nowhere. Every hook of Claude Code's
//! settings files is resolved without running it: the command is split like a POSIX shell,
//! variables are expanded, and the target is the first word, the script of a known
//! interpreter, or a program on `PATH`. A target that is not there is `broken-hook`; a script
//! that exists but is not executable is `suspect-hook`; a command that cannot be judged
//! statically (substitution, operators, unknown variables) is `unverified-hook`. Only the
//! first class will ever be offered for removal (T331.5). Nothing here edits a file, and
//! nothing reaches the machine except through [`super::probe`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use super::probe::{Env, Fs, PathKind, Which};
use crate::config::Config;

/// One finding of a doctor config check. The list is shared by every T331 detector.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct Problem {
    /// `broken-hook`, `suspect-hook`, `unverified-hook` or `unreadable-config`.
    pub kind: &'static str,
    pub agent: &'static str,
    /// The config file the entry lives in.
    pub source: String,
    /// The key path of the entry inside `source`.
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// The command as written, never expanded.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    pub detail: String,
    /// Whether `doctor --fix` may remove it (T331.5): only `broken-hook`.
    pub fixable: bool,
}

/// What a hook command may rely on besides the machine.
#[derive(Clone, Copy)]
struct Scope<'a> {
    project: &'a Path,
    /// `${CLAUDE_PLUGIN_ROOT}`, known only inside a plugin's own `hooks.json`.
    plugin_root: Option<&'a Path>,
    /// Whether a relative path resolves against `project`. Only Claude Code documents that;
    /// for any other host a relative path is not judged, since guessing its base would offer
    /// a working hook for removal.
    relative_ok: bool,
}

/// The machine, as the checks see it.
pub struct Probes<'a> {
    pub fs: &'a dyn Fs,
    pub env: &'a dyn Env,
    pub which: &'a dyn Which,
}

#[derive(Debug, PartialEq)]
struct Entry {
    event: String,
    matcher: Option<String>,
    command: String,
    key: String,
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Ok,
    Broken(String),
    Suspect(String),
    Unverified(String),
}

/// Programs that run a script given as an argument.
const INTERPRETERS: &[&str] = &[
    "bash", "sh", "zsh", "node", "python", "python3", "deno", "bun", "ruby", "pwsh", "tsx",
];

/// Shell builtins a hook may name directly; they need no file.
const BUILTINS: &[&str] = &[
    ":", ".", "[", "cd", "command", "echo", "exit", "export", "false", "printf", "pwd", "read",
    "set", "source", "test", "true", "type", "unset",
];

/// The Claude Code settings files in load order: user, user local, project, project local.
fn sources(cfg: &Config, project: Option<&Path>) -> Vec<PathBuf> {
    let user = cfg.doctor.settings_path.clone();
    let mut out = vec![user.clone(), user.with_file_name("settings.local.json")];
    if let Some(p) = project {
        let dir = p.join(".claude");
        out.push(dir.join("settings.json"));
        out.push(dir.join("settings.local.json"));
    }
    // No file twice: the working directory may be `$HOME`.
    let mut seen = BTreeSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// Every command hook of a settings document: `hooks.<Event>[].{matcher, hooks[].command}`,
/// plus the flat `hooks.<Event>[].command` shape `doctor`'s hook count also accepts.
fn entries(doc: &Value) -> Vec<Entry> {
    let mut out = Vec::new();
    let Some(events) = doc.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for (event, groups) in events {
        for (gi, group) in groups.as_array().into_iter().flatten().enumerate() {
            let matcher = group
                .get("matcher")
                .and_then(Value::as_str)
                .map(String::from);
            let mut push = |hook: &Value, key: String| {
                let is_command = hook
                    .get("type")
                    .and_then(Value::as_str)
                    .is_none_or(|t| t == "command");
                if let (true, Some(command)) =
                    (is_command, hook.get("command").and_then(Value::as_str))
                {
                    out.push(Entry {
                        event: event.clone(),
                        matcher: matcher.clone(),
                        command: command.to_string(),
                        key,
                    });
                }
            };
            match group.get("hooks").and_then(Value::as_array) {
                Some(hooks) => {
                    for (hi, hook) in hooks.iter().enumerate() {
                        push(hook, format!("hooks.{event}[{gi}].hooks[{hi}]"));
                    }
                }
                None => push(group, format!("hooks.{event}[{gi}]")),
            }
        }
    }
    out
}

/// Syntax a static check cannot judge: substitution, operators, subshells, line breaks.
fn shell_syntax(raw: &str) -> Option<&'static str> {
    if raw.contains("$(") || raw.contains('`') {
        Some("command substitution")
    } else if raw.contains(['|', '&', ';', '<', '>', '(', ')', '\n']) {
        Some("shell operators")
    } else if raw.split_whitespace().next() == Some("eval") {
        Some("eval")
    } else {
        None
    }
}

/// `word` with `~`, `$NAME` and `${NAME}` expanded. `Err` names what could not be: an unset
/// variable, or one only a plugin install defines.
fn expand(word: &str, p: &Probes, scope: &Scope) -> Result<String, String> {
    let mut out = String::new();
    let rest = if word == "~" || word.starts_with("~/") {
        let home = p.env.home().ok_or("HOME")?;
        out.push_str(&home.to_string_lossy());
        &word[1..]
    } else {
        word
    };
    let mut chars = rest.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let tail = &rest[i + 1..];
        let (name, len) = match tail.strip_prefix('{') {
            Some(inner) => {
                let end = inner.find('}').ok_or("$")?;
                (&inner[..end], end + 2)
            }
            None => {
                let end = tail
                    .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                    .unwrap_or(tail.len());
                (&tail[..end], end)
            }
        };
        if name.is_empty() || name.starts_with(|ch: char| ch.is_ascii_digit()) {
            return Err("$".into());
        }
        let value = match name {
            "HOME" => p.env.home().map(|h| h.to_string_lossy().into_owned()),
            "CLAUDE_PROJECT_DIR" => Some(scope.project.to_string_lossy().into_owned()),
            "CLAUDE_PLUGIN_ROOT" => scope.plugin_root.map(|r| r.to_string_lossy().into_owned()),
            other => p.env.var(other),
        };
        out.push_str(&value.ok_or_else(|| name.to_string())?);
        for _ in 0..len {
            chars.next();
        }
    }
    Ok(out)
}

fn base_name(program: &str) -> String {
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    name.strip_suffix(".exe").unwrap_or(name).to_lowercase()
}

/// What a command runs: the path or program to look up, and whether the shell would exec it
/// directly (so its own exec bit matters) rather than an interpreter reading it.
fn target(words: &[String]) -> Result<(String, bool), Verdict> {
    let program = &words[0];
    let base = base_name(program);
    let args = &words[1..];
    let first_arg = |args: &[String]| args.iter().find(|a| !a.starts_with('-')).cloned();
    if INTERPRETERS.contains(&base.as_str()) {
        if args
            .iter()
            .any(|a| matches!(a.as_str(), "-c" | "-e" | "--eval" | "-p" | "--print"))
        {
            return Err(Verdict::Unverified("an inline script, not a file".into()));
        }
        let args: Vec<String> = match base.as_str() {
            "deno" | "bun" => args.iter().filter(|a| *a != "run").cloned().collect(),
            _ => args.to_vec(),
        };
        return first_arg(&args)
            .map(|script| (script, false))
            .ok_or_else(|| Verdict::Unverified("no script argument".into()));
    }
    // Launchers that hand the rest of the line to another program.
    let rest_after = |word: &str| {
        args.iter()
            .skip_while(|a| *a != word)
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
    };
    match (base.as_str(), first_arg(args).as_deref()) {
        ("npx", Some("tsx")) => target(&[&["tsx".to_string()][..], &rest_after("tsx")].concat()),
        ("uv", Some("run")) => {
            let rest = rest_after("run");
            if rest.is_empty() {
                return Err(Verdict::Unverified("no script argument".into()));
            }
            target(&rest).map(|(t, _)| (t, false))
        }
        _ => Ok((program.clone(), true)),
    }
}

fn path_like(t: &str) -> bool {
    t.contains(['/', '\\']) || t.starts_with(['~', '.'])
}

fn classify(command: &str, p: &Probes, scope: &Scope) -> Verdict {
    let raw = command.trim();
    if raw.is_empty() {
        return Verdict::Unverified("empty command".into());
    }
    if let Some(why) = shell_syntax(raw) {
        return Verdict::Unverified(format!("{why}: cannot be checked without running it"));
    }
    let Some(words) = shlex::split(raw) else {
        return Verdict::Unverified("unbalanced quotes".into());
    };
    let skip = words
        .iter()
        .take_while(|w| {
            w.split_once('=').is_some_and(|(k, _)| {
                !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            })
        })
        .count();
    if skip == words.len() {
        return Verdict::Unverified("only variable assignments".into());
    }
    let mut expanded = Vec::new();
    for w in &words[skip..] {
        match expand(w, p, scope) {
            Ok(x) => expanded.push(x),
            Err(name) if name == "CLAUDE_PLUGIN_ROOT" => {
                return Verdict::Unverified(
                    "`CLAUDE_PLUGIN_ROOT` is only known inside a plugin's own hooks.json".into(),
                );
            }
            Err(name) => return Verdict::Unverified(format!("`{name}` is not set here")),
        }
    }
    let (target, direct) = match target(&expanded) {
        Ok(t) => t,
        Err(v) => return v,
    };
    if !path_like(&target) && direct {
        return if BUILTINS.contains(&target.as_str()) || p.which.find(&target).is_some() {
            Verdict::Ok
        } else {
            Verdict::Broken(format!("`{target}` not on PATH"))
        };
    }
    let path = if Path::new(&target).is_absolute() {
        PathBuf::from(&target)
    } else if scope.relative_ok {
        scope
            .project
            .join(target.strip_prefix("./").unwrap_or(&target))
    } else {
        return Verdict::Unverified(
            "relative path: this host's base directory is not known".into(),
        );
    };
    if let Some(volume) = unmounted_volume(&path, p.fs) {
        return Verdict::Broken(format!("path is on {volume}, which is not mounted"));
    }
    match p.fs.kind(&path) {
        PathKind::Missing => Verdict::Broken(format!("file not found: {}", path.display())),
        PathKind::DanglingSymlink(to) => {
            Verdict::Broken(format!("dangling symlink to {}", to.display()))
        }
        PathKind::Dir => {
            Verdict::Broken(format!("{} is a directory, not a script", path.display()))
        }
        PathKind::File { executable: false } if direct => Verdict::Suspect(format!(
            "{} is not executable: run `chmod +x`",
            path.display()
        )),
        PathKind::File { .. } => Verdict::Ok,
    }
}

/// `/Volumes/X` when `path` sits on a macOS volume that is not mounted.
fn unmounted_volume(path: &Path, fs: &dyn Fs) -> Option<String> {
    let mut parts = path.components();
    let root = parts.nth(1)?; // the component after `/`
    let volume = parts.next()?;
    if root.as_os_str() != "Volumes" || path.parent().is_none() {
        return None;
    }
    let mount = Path::new("/Volumes").join(volume.as_os_str());
    (fs.kind(&mount) == PathKind::Missing).then(|| mount.display().to_string())
}

/// Check every hook of Claude Code's settings files and enabled plugins, and of the JSON
/// config files the other hosts' installers write. A file that does not exist is skipped; one
/// that cannot be read or parsed is reported and left alone.
pub fn check(cfg: &Config, p: &Probes) -> Vec<Problem> {
    let project = p.env.cwd().unwrap_or_default();
    let claude = Scope {
        project: &project,
        plugin_root: None,
        relative_ok: true,
    };
    let other = Scope {
        relative_ok: false,
        ..claude
    };
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut enabled = BTreeSet::new();
    let user_sources = sources(
        cfg,
        Some(project.as_path()).filter(|d| !d.as_os_str().is_empty()),
    );
    for source in &user_sources {
        if let Some(doc) = scan(p, "claude", source, &claude, false, &mut out) {
            enabled.extend(enabled_plugins(&doc));
        }
    }
    seen.extend(user_sources);
    for (agent, source) in host_sources(cfg) {
        if seen.insert(source.clone()) {
            scan(p, agent, &source, &other, false, &mut out);
        }
    }
    plugins(cfg, p, &enabled, &claude, &mut out);
    out
}

/// The JSON files every host but Claude Code keeps hooks in, as its installer names them, once
/// each (Cursor's variants share theirs). Other formats wait for T331.8.
fn host_sources(cfg: &Config) -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    for agent in crate::agents::HOSTS
        .iter()
        .filter(|id| **id != "claude")
        .filter_map(|id| crate::agents::host(id))
    {
        for v in agent.variants() {
            for path in agent.files(cfg, v.kind) {
                let json = path
                    .extension()
                    .is_some_and(|e| e == "json" || e == "jsonc");
                if json && !out.iter().any(|(_, seen)| *seen == path) {
                    out.push((agent.id(), path));
                }
            }
        }
    }
    out
}

/// `enabledPlugins` of a settings document: the ids switched on.
fn enabled_plugins(doc: &Value) -> impl Iterator<Item = String> + '_ {
    doc.get("enabledPlugins")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(_, on)| on.as_bool() == Some(true))
        .map(|(id, _)| id.clone())
}

/// The hooks of every enabled Claude plugin, with `${CLAUDE_PLUGIN_ROOT}` resolved against its
/// install directory. A plugin that is enabled but whose directory is gone is reported as
/// stale. These files belong to the plugin, so nothing found in them is ever fixable.
fn plugins(
    cfg: &Config,
    p: &Probes,
    enabled: &BTreeSet<String>,
    claude: &Scope,
    out: &mut Vec<Problem>,
) {
    let index = crate::agents::claude::config_dir(cfg).join("plugins/installed_plugins.json");
    let Some(root) =
        p.fs.read(&index)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
    else {
        return;
    };
    let Some(plugins) = root.get("plugins").and_then(Value::as_object) else {
        return;
    };
    for id in plugins.keys().filter(|id| enabled.contains(*id)) {
        for (i, dir) in super::plugin_install_paths(&root, id).iter().enumerate() {
            let dir = PathBuf::from(dir);
            if p.fs.kind(&dir) != PathKind::Dir {
                out.push(Problem {
                    path: format!("plugins.{id}[{i}]"),
                    detail: format!(
                        "`{id}` is enabled but its install directory {} is gone",
                        dir.display()
                    ),
                    ..problem("stale-plugin", "claude", &index)
                });
                continue;
            }
            let scope = Scope {
                plugin_root: Some(&dir),
                ..*claude
            };
            scan(
                p,
                "claude",
                &dir.join("hooks/hooks.json"),
                &scope,
                true,
                out,
            );
        }
    }
}

/// One config file: its hooks classified into `out`. `managed` marks a file that is not the
/// user's to edit, so nothing in it is fixable. Returns the parsed document.
fn scan(
    p: &Probes,
    agent: &'static str,
    source: &Path,
    scope: &Scope,
    managed: bool,
    out: &mut Vec<Problem>,
) -> Option<Value> {
    let raw = match p.fs.read(source) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            out.push(unreadable(agent, source, &e.to_string()));
            return None;
        }
    };
    let doc = match crate::agents::jsonc::parse(&raw) {
        Ok(doc) => doc,
        Err(e) => {
            out.push(unreadable(agent, source, &e.to_string()));
            return None;
        }
    };
    for e in entries(&doc) {
        let (kind, detail, fixable) = match classify(&e.command, p, scope) {
            Verdict::Ok => continue,
            Verdict::Broken(d) => ("broken-hook", d, !managed),
            Verdict::Suspect(d) => ("suspect-hook", d, false),
            Verdict::Unverified(d) => ("unverified-hook", d, false),
        };
        out.push(Problem {
            path: e.key,
            event: e.event,
            matcher: e.matcher,
            command: e.command,
            detail,
            fixable,
            ..problem(kind, agent, source)
        });
    }
    Some(doc)
}

/// A finding with only its identity set.
fn problem(kind: &'static str, agent: &'static str, source: &Path) -> Problem {
    Problem {
        kind,
        agent,
        source: source.display().to_string(),
        path: String::new(),
        event: String::new(),
        matcher: None,
        command: String::new(),
        detail: String::new(),
        fixable: false,
    }
}

fn unreadable(agent: &'static str, source: &Path, why: &str) -> Problem {
    Problem {
        detail: format!("cannot read {}: {why}", source.display()),
        ..problem("unreadable-config", agent, source)
    }
}

/// [`check`] against this machine.
pub fn check_real(cfg: &Config) -> Vec<Problem> {
    check(
        cfg,
        &Probes {
            fs: &super::probe::RealFs,
            env: &super::probe::RealEnv,
            which: &super::probe::RealWhich,
        },
    )
}

/// The `hooks check` lines of the doctor text: grouped by class, or "none found".
pub fn render(problems: &[Problem]) -> String {
    let hook_problems: Vec<&Problem> = problems
        .iter()
        .filter(|p| {
            p.kind.ends_with("-hook") || matches!(p.kind, "unreadable-config" | "stale-plugin")
        })
        .collect();
    if hook_problems.is_empty() {
        return "hooks check none found\n".into();
    }
    let mut out = String::from("hooks check\n");
    for (kind, title) in [
        ("broken-hook", "broken"),
        ("suspect-hook", "suspect"),
        ("unverified-hook", "cannot verify"),
        ("stale-plugin", "stale plugin"),
        ("unreadable-config", "unreadable"),
    ] {
        for p in hook_problems.iter().filter(|p| p.kind == kind) {
            let matcher = p
                .matcher
                .as_deref()
                .map(|m| format!("[{m}]"))
                .unwrap_or_default();
            if matches!(kind, "unreadable-config" | "stale-plugin") {
                out.push_str(&format!("  {title} {} {}\n", p.agent, p.detail));
                continue;
            }
            let note = if p.fixable {
                " (can be cleaned up)"
            } else {
                ""
            };
            out.push_str(&format!(
                "  {title} {} {}{matcher} `{}`: {}{note}\n    {} {}\n",
                p.agent, p.event, p.command, p.detail, p.source, p.path
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io;

    /// An in-memory machine: files, kinds, programs on `PATH` and the environment.
    #[derive(Default, Clone)]
    struct Mock {
        files: BTreeMap<PathBuf, String>,
        kinds: BTreeMap<PathBuf, PathKind>,
        path: BTreeMap<String, PathBuf>,
        env: BTreeMap<String, String>,
        unreadable: BTreeSet<PathBuf>,
    }

    impl Fs for Mock {
        fn read(&self, path: &Path) -> io::Result<String> {
            if self.unreadable.contains(path) {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
        fn kind(&self, path: &Path) -> PathKind {
            self.kinds.get(path).cloned().unwrap_or(PathKind::Missing)
        }
    }
    impl Env for Mock {
        fn var(&self, name: &str) -> Option<String> {
            self.env.get(name).cloned()
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

    impl Mock {
        fn script(&mut self, path: &str, executable: bool) {
            self.kinds
                .insert(path.into(), PathKind::File { executable });
        }
        fn settings(&mut self, path: &str, commands: &[&str]) {
            let hooks: Vec<Value> = commands
                .iter()
                .map(|c| serde_json::json!({"type": "command", "command": c}))
                .collect();
            let doc =
                serde_json::json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": hooks}]}});
            self.files.insert(path.into(), doc.to_string());
        }
    }

    fn cfg() -> Config {
        let mut c = Config::default();
        c.doctor.settings_path = "/h/.claude/settings.json".into();
        c.setup.claude.settings_path = "/h/.claude/settings.json".into();
        c.setup.cursor.hooks_path = "/h/.cursor/hooks.json".into();
        c.setup.gemini.dir = "/h/.gemini".into();
        c
    }

    fn run(m: &Mock, commands: &[&str]) -> Vec<Problem> {
        let mut m2 = m.clone();
        m2.settings("/h/.claude/settings.json", commands);
        check(
            &cfg(),
            &Probes {
                fs: &m2,
                env: &m2,
                which: &m2,
            },
        )
    }

    fn verdicts(m: &Mock, commands: &[&str]) -> Vec<(&'static str, String)> {
        run(m, commands)
            .into_iter()
            .map(|p| (p.kind, p.detail))
            .collect()
    }

    #[test]
    fn a_missing_script_is_broken_and_fixable() {
        let found = run(&Mock::default(), &["~/scripts/old-guard.sh"]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind, "broken-hook");
        assert!(found[0].fixable);
        assert_eq!(found[0].detail, "file not found: /h/scripts/old-guard.sh");
        assert_eq!(found[0].event, "PreToolUse");
        assert_eq!(found[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(found[0].path, "hooks.PreToolUse[0].hooks[0]");
    }

    #[test]
    fn a_dangling_symlink_a_directory_and_an_unmounted_volume_are_broken() {
        let mut m = Mock::default();
        m.kinds.insert(
            "/h/link.sh".into(),
            PathKind::DanglingSymlink("/Volumes/X/hook.js".into()),
        );
        m.kinds.insert("/h/dir".into(), PathKind::Dir);
        let v = verdicts(&m, &["/h/link.sh", "/h/dir", "/Volumes/X/hook.js"]);
        assert_eq!(
            v[0],
            (
                "broken-hook",
                "dangling symlink to /Volumes/X/hook.js".into()
            )
        );
        assert_eq!(v[1].0, "broken-hook");
        assert!(v[1].1.contains("is a directory"));
        assert_eq!(
            v[2],
            (
                "broken-hook",
                "path is on /Volumes/X, which is not mounted".into()
            )
        );
    }

    #[test]
    fn every_interpreter_with_a_missing_script_is_broken_and_with_a_present_one_is_fine() {
        let mut m = Mock::default();
        m.script("/proj/ok.js", false);
        for interp in [
            "bash", "sh", "zsh", "node", "python", "python3", "deno run", "bun", "ruby", "pwsh",
            "npx tsx", "uv run",
        ] {
            let missing = verdicts(&m, &[&format!("{interp} /proj/gone.js")]);
            assert_eq!(missing.len(), 1, "{interp}");
            assert_eq!(missing[0].0, "broken-hook", "{interp}");
            assert!(
                missing[0].1.contains("/proj/gone.js"),
                "{interp}: {missing:?}"
            );
            assert!(
                verdicts(&m, &[&format!("{interp} /proj/ok.js")]).is_empty(),
                "{interp}"
            );
        }
    }

    #[test]
    fn a_program_is_looked_up_on_path_and_a_builtin_needs_no_file() {
        let mut m = Mock::default();
        m.path.insert("jq".into(), "/usr/bin/jq".into());
        let v = verdicts(&m, &["jq .", "foo --x", "echo hi", "true"]);
        assert_eq!(v, vec![("broken-hook", "`foo` not on PATH".to_string())]);
    }

    #[test]
    fn variables_expand_and_relative_paths_resolve_against_the_project() {
        let mut m = Mock::default();
        m.script("/proj/.claude/hooks/a.sh", true);
        m.script("/h/b.sh", true);
        m.script("/proj/rel/c.sh", true);
        m.env.insert("MY_DIR".into(), "/proj/rel".into());
        let v = verdicts(
            &m,
            &[
                "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/a.sh",
                "$HOME/b.sh",
                "${HOME}/b.sh",
                "./rel/c.sh",
                "$MY_DIR/c.sh",
            ],
        );
        assert!(v.is_empty(), "{v:?}");
    }

    #[test]
    fn a_quoted_path_with_spaces_is_one_word() {
        let mut m = Mock::default();
        m.script("/h/My Scripts/guard.sh", true);
        assert!(verdicts(&m, &["\"/h/My Scripts/guard.sh\""]).is_empty());
        let gone = verdicts(&m, &["'/h/Other Scripts/guard.sh'"]);
        assert_eq!(gone[0].1, "file not found: /h/Other Scripts/guard.sh");
    }

    #[test]
    fn leading_variable_assignments_are_skipped() {
        let mut m = Mock::default();
        m.script("/h/a.sh", true);
        assert!(verdicts(&m, &["RTOK_X=1 /h/a.sh"]).is_empty());
    }

    #[test]
    fn a_script_that_exists_but_is_not_executable_is_suspect_only_when_run_directly() {
        let mut m = Mock::default();
        m.script("/h/a.sh", false);
        let direct = run(&m, &["/h/a.sh"]);
        assert_eq!(direct[0].kind, "suspect-hook");
        assert!(!direct[0].fixable);
        assert!(direct[0].detail.contains("chmod +x"));
        m.path.insert("bash".into(), "/bin/bash".into());
        assert!(verdicts(&m, &["bash /h/a.sh"]).is_empty());
    }

    #[test]
    fn what_cannot_be_judged_statically_is_unverified_and_never_fixable() {
        let m = Mock::default();
        for cmd in [
            "$(which foo)",
            "`which foo`",
            "eval \"$X\"",
            "cat x | sh",
            "a && b",
            "$UNSET_THING/run.sh",
            "${CLAUDE_PLUGIN_ROOT}/hooks/run.sh",
            "bash -c 'echo hi'",
            "bash",
            "",
            "'unbalanced",
        ] {
            let found = run(&m, &[cmd]);
            assert_eq!(found.len(), 1, "{cmd}");
            assert_eq!(found[0].kind, "unverified-hook", "{cmd}: {found:?}");
            assert!(!found[0].fixable, "{cmd}");
        }
    }

    #[test]
    fn user_local_and_project_files_are_all_read() {
        let mut m = Mock::default();
        m.settings("/h/.claude/settings.json", &["/h/a"]);
        m.settings("/h/.claude/settings.local.json", &["/h/b"]);
        m.settings("/proj/.claude/settings.json", &["/h/c"]);
        m.settings("/proj/.claude/settings.local.json", &["/h/d"]);
        let found = check(
            &cfg(),
            &Probes {
                fs: &m,
                env: &m,
                which: &m,
            },
        );
        let mut sources: Vec<&str> = found.iter().map(|p| p.source.as_str()).collect();
        sources.sort();
        assert_eq!(
            sources,
            vec![
                "/h/.claude/settings.json",
                "/h/.claude/settings.local.json",
                "/proj/.claude/settings.json",
                "/proj/.claude/settings.local.json"
            ]
        );
    }

    #[test]
    fn jsonc_comments_and_the_flat_hook_shape_are_read() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.claude/settings.json".into(),
            "{ // user hooks\n \"hooks\": { \"Stop\": [ { \"command\": \"/h/gone.sh\", }, ], },\n}"
                .into(),
        );
        let found = check(
            &cfg(),
            &Probes {
                fs: &m,
                env: &m,
                which: &m,
            },
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, "hooks.Stop[0]");
        assert_eq!(found[0].event, "Stop");
    }

    #[test]
    fn an_unparsable_or_unreadable_file_is_reported_and_a_missing_one_is_not() {
        let mut m = Mock::default();
        m.files
            .insert("/h/.claude/settings.json".into(), "{ nope".into());
        m.files
            .insert("/h/.claude/settings.local.json".into(), "{}".into());
        m.unreadable.insert("/proj/.claude/settings.json".into());
        let found = check(
            &cfg(),
            &Probes {
                fs: &m,
                env: &m,
                which: &m,
            },
        );
        let kinds: Vec<&str> = found.iter().map(|p| p.kind).collect();
        assert_eq!(kinds, vec!["unreadable-config", "unreadable-config"]);
        assert!(
            found
                .iter()
                .all(|p| !p.fixable && p.detail.starts_with("cannot read "))
        );
    }

    #[test]
    fn a_file_without_hooks_has_no_problem_and_non_command_hooks_are_ignored() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.claude/settings.json".into(),
            r#"{"hooks": {"Stop": [{"hooks": [{"type": "prompt", "prompt": "x"}]}]}}"#.into(),
        );
        m.files.insert(
            "/h/.claude/settings.local.json".into(),
            "{\"model\": \"x\"}".into(),
        );
        assert!(
            check(
                &cfg(),
                &Probes {
                    fs: &m,
                    env: &m,
                    which: &m
                }
            )
            .is_empty()
        );
    }

    fn check_with(m: &Mock) -> Vec<Problem> {
        check(
            &cfg(),
            &Probes {
                fs: m,
                env: m,
                which: m,
            },
        )
    }

    #[test]
    fn another_hosts_json_hooks_are_checked_under_its_own_name_and_a_shared_file_once() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.cursor/hooks.json".into(),
            r#"{"version": 1, "hooks": {"beforeShellExecution": [{"command": "/h/gone.sh"}]}}"#
                .into(),
        );
        m.files.insert(
            "/h/.gemini/settings.json".into(),
            r#"{"hooks": {"BeforeTool": [{"matcher": "run_shell_command", "hooks": [{"type": "command", "command": "/h/gone2.sh"}]}]}}"#
                .into(),
        );
        let found = check_with(&m);
        let by: Vec<(&str, &str, bool)> = found
            .iter()
            .map(|p| (p.agent, p.event.as_str(), p.fixable))
            .collect();
        assert_eq!(
            by,
            vec![
                ("cursor", "beforeShellExecution", true),
                ("gemini", "BeforeTool", true)
            ]
        );
        assert_eq!(found[1].matcher.as_deref(), Some("run_shell_command"));
    }

    #[test]
    fn a_relative_path_is_not_judged_for_a_host_that_does_not_document_its_base() {
        let mut m = Mock::default();
        m.files.insert(
            "/h/.cursor/hooks.json".into(),
            r#"{"hooks": {"stop": [{"command": "./hooks/a.sh"}, {"command": "/h/gone.sh"}]}}"#
                .into(),
        );
        let found = check_with(&m);
        assert_eq!(found[0].kind, "unverified-hook");
        assert!(found[0].detail.starts_with("relative path"));
        assert!(!found[0].fixable);
        assert_eq!(found[1].kind, "broken-hook");
        // Claude Code runs hooks in the project directory.
        m.settings("/h/.claude/settings.json", &["./hooks/a.sh"]);
        let claude = check_with(&m);
        assert_eq!(claude[0].agent, "claude");
        assert_eq!(claude[0].detail, "file not found: /proj/hooks/a.sh");
    }

    fn plugin_home(m: &mut Mock, enabled: bool, install_dir: bool, hooks: &str) {
        m.files.insert(
            "/h/.claude/plugins/installed_plugins.json".into(),
            r#"{"plugins": {"demo@mkt": [{"installPath": "/h/plug/demo"}], "off@mkt": [{"installPath": "/h/plug/off"}]}}"#
                .into(),
        );
        m.files.insert(
            "/h/.claude/settings.json".into(),
            serde_json::json!({"enabledPlugins": {"demo@mkt": enabled, "off@mkt": false}})
                .to_string(),
        );
        if install_dir {
            m.kinds.insert("/h/plug/demo".into(), PathKind::Dir);
            m.files
                .insert("/h/plug/demo/hooks/hooks.json".into(), hooks.into());
        }
    }

    #[test]
    fn a_plugin_hook_resolves_the_plugin_root_and_is_never_fixable() {
        let mut m = Mock::default();
        let hooks = r#"{"hooks": {"SessionStart": [{"hooks": [
            {"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/scripts/ok.sh"},
            {"type": "command", "command": "bash \"${CLAUDE_PLUGIN_ROOT}/scripts/gone.sh\" start"}]}]}}"#;
        plugin_home(&mut m, true, true, hooks);
        m.script("/h/plug/demo/scripts/ok.sh", true);
        // A disabled plugin is not loaded, so its hooks are not checked.
        m.files.insert(
            "/h/plug/off/hooks/hooks.json".into(),
            r#"{"hooks": {"Stop": [{"command": "/h/never.sh"}]}}"#.into(),
        );
        let found = check_with(&m);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, "broken-hook");
        assert_eq!(
            found[0].detail,
            "file not found: /h/plug/demo/scripts/gone.sh"
        );
        assert_eq!(found[0].source, "/h/plug/demo/hooks/hooks.json");
        assert!(!found[0].fixable);
    }

    #[test]
    fn an_enabled_plugin_whose_directory_is_gone_is_stale_and_a_disabled_one_is_ignored() {
        let mut m = Mock::default();
        plugin_home(&mut m, true, false, "");
        let found = check_with(&m);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, "stale-plugin");
        assert_eq!(found[0].path, "plugins.demo@mkt[0]");
        assert!(found[0].detail.contains("/h/plug/demo is gone"));
        assert!(!found[0].fixable);
        plugin_home(&mut m, false, false, "");
        assert!(check_with(&m).is_empty());
    }

    #[test]
    fn plugin_root_outside_a_plugin_is_still_unverified() {
        let m = Mock::default();
        let found = run(&m, &["${CLAUDE_PLUGIN_ROOT}/x.sh"]);
        assert_eq!(found[0].kind, "unverified-hook");
    }

    #[test]
    fn a_foreign_non_json_host_file_is_left_alone() {
        let mut m = Mock::default();
        m.files
            .insert("/h/.cursor/hooks.json".into(), "{ nope".into());
        let found = check_with(&m);
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].kind, found[0].agent),
            ("unreadable-config", "cursor")
        );
    }

    #[test]
    fn the_text_groups_by_class_and_says_none_found() {
        assert_eq!(render(&[]), "hooks check none found\n");
        let found = run(&Mock::default(), &["/h/gone.sh", "$(x)"]);
        let text = render(&found);
        assert!(text.starts_with("hooks check\n"), "{text}");
        assert!(text.contains("broken claude PreToolUse[Bash] `/h/gone.sh`: file not found: /h/gone.sh (can be cleaned up)"), "{text}");
        assert!(
            text.contains("cannot verify claude PreToolUse[Bash] `$(x)`"),
            "{text}"
        );
        assert!(
            text.contains("/h/.claude/settings.json hooks.PreToolUse[0].hooks[0]"),
            "{text}"
        );
    }

    /// The doctor modules reach files, the environment and `PATH` only through `probe`.
    #[test]
    fn only_the_probe_module_touches_the_machine() {
        let banned = [
            concat!("std::", "fs"),
            concat!("std::", "env"),
            concat!("which", "::"),
            concat!("fs", "::read"),
        ];
        for (name, src) in [("hooks.rs", include_str!("hooks.rs"))] {
            let code = src.split("#[cfg(test)]").next().unwrap_or(src);
            for b in banned {
                assert!(
                    !code.contains(b),
                    "{name} calls `{b}` directly; go through probe"
                );
            }
        }
    }
}
