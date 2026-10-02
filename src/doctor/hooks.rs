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
fn expand(word: &str, p: &Probes, project: &Path) -> Result<String, String> {
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
            "CLAUDE_PROJECT_DIR" => Some(project.to_string_lossy().into_owned()),
            "CLAUDE_PLUGIN_ROOT" => None,
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

fn classify(command: &str, p: &Probes, project: &Path) -> Verdict {
    let raw = command.trim();
    if raw.is_empty() {
        return Verdict::Unverified("empty command".into());
    }
    if let Some(why) = shell_syntax(raw) {
        return Verdict::Unverified(format!("{why}: cannot be checked without running it"));
    }
    // POSIX word splitting reads `\` as an escape, so an unquoted Windows path would be mangled
    // into a path that does not exist and a working hook reported broken. Windows rules: T331.9.
    if cfg!(windows) && raw.contains('\\') && !raw.contains(['\'', '"']) {
        return Verdict::Unverified("Windows paths are not checked yet".into());
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
        match expand(w, p, project) {
            Ok(x) => expanded.push(x),
            Err(name) if name == "CLAUDE_PLUGIN_ROOT" => {
                return Verdict::Unverified(
                    "`CLAUDE_PLUGIN_ROOT` is only known for plugin hooks".into(),
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
    } else {
        project.join(&target)
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

/// `/Volumes/X` when `path` sits on a macOS volume that is not mounted. `/Volumes` exists only
/// on macOS; elsewhere such a path is an ordinary path and the missing-file rule covers it.
#[cfg(target_os = "macos")]
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

#[cfg(not(target_os = "macos"))]
fn unmounted_volume(_path: &Path, _fs: &dyn Fs) -> Option<String> {
    None
}

/// Check every hook of Claude Code's settings files. A file that does not exist is skipped; one
/// that cannot be read or parsed is reported and left alone.
pub fn check(cfg: &Config, p: &Probes) -> Vec<Problem> {
    let project = p.env.cwd().unwrap_or_default();
    let mut out = Vec::new();
    for source in sources(
        cfg,
        Some(project.as_path()).filter(|d| !d.as_os_str().is_empty()),
    ) {
        let raw = match p.fs.read(&source) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                out.push(unreadable(&source, &e.to_string()));
                continue;
            }
        };
        let doc = match crate::agents::jsonc::parse(&raw) {
            Ok(doc) => doc,
            Err(e) => {
                out.push(unreadable(&source, &e.to_string()));
                continue;
            }
        };
        for e in entries(&doc) {
            let (kind, detail, fixable) = match classify(&e.command, p, &project) {
                Verdict::Ok => continue,
                Verdict::Broken(d) => ("broken-hook", d, true),
                Verdict::Suspect(d) => ("suspect-hook", d, false),
                Verdict::Unverified(d) => ("unverified-hook", d, false),
            };
            out.push(Problem {
                kind,
                agent: "claude",
                source: source.display().to_string(),
                path: e.key,
                event: e.event,
                matcher: e.matcher,
                command: e.command,
                detail,
                fixable,
            });
        }
    }
    out
}

fn unreadable(source: &Path, why: &str) -> Problem {
    Problem {
        kind: "unreadable-config",
        agent: "claude",
        source: source.display().to_string(),
        path: String::new(),
        event: String::new(),
        matcher: None,
        command: String::new(),
        detail: format!("cannot read {}: {why}", source.display()),
        fixable: false,
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
        .filter(|p| p.kind.ends_with("-hook") || p.kind == "unreadable-config")
        .collect();
    if hook_problems.is_empty() {
        return "hooks check none found\n".into();
    }
    let mut out = String::from("hooks check\n");
    for (kind, title, note) in [
        ("broken-hook", "broken", " (can be cleaned up)"),
        ("suspect-hook", "suspect", ""),
        ("unverified-hook", "cannot verify", ""),
        ("unreadable-config", "unreadable", ""),
    ] {
        for p in hook_problems.iter().filter(|p| p.kind == kind) {
            let matcher = p
                .matcher
                .as_deref()
                .map(|m| format!("[{m}]"))
                .unwrap_or_default();
            if kind == "unreadable-config" {
                out.push_str(&format!("  {title} {}\n", p.detail));
                continue;
            }
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

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
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

    /// An absolute path on every OS, quoted so POSIX word splitting keeps a Windows `\`.
    fn quoted_abs(name: &str) -> (PathBuf, String) {
        let path = std::env::temp_dir().join("rtok-hooks-test").join(name);
        let cmd = format!("'{}'", path.display());
        (path, cmd)
    }

    #[test]
    fn a_dangling_symlink_and_a_directory_are_broken() {
        let mut m = Mock::default();
        let (link, link_cmd) = quoted_abs("link.sh");
        let (dir, dir_cmd) = quoted_abs("dir");
        let target = std::env::temp_dir().join("rtok-hooks-test").join("gone.js");
        m.kinds
            .insert(link, PathKind::DanglingSymlink(target.clone()));
        m.kinds.insert(dir, PathKind::Dir);
        let v = verdicts(&m, &[&link_cmd, &dir_cmd]);
        assert_eq!(
            v[0],
            (
                "broken-hook",
                format!("dangling symlink to {}", target.display())
            )
        );
        assert_eq!(v[1].0, "broken-hook");
        assert!(v[1].1.contains("is a directory"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_path_on_an_unmounted_volume_is_broken() {
        let v = verdicts(&Mock::default(), &["/Volumes/X/hook.js"]);
        assert_eq!(
            v[0],
            (
                "broken-hook",
                "path is on /Volumes/X, which is not mounted".into()
            )
        );
    }

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
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

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
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

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
    #[test]
    fn a_quoted_path_with_spaces_is_one_word() {
        let mut m = Mock::default();
        m.script("/h/My Scripts/guard.sh", true);
        assert!(verdicts(&m, &["\"/h/My Scripts/guard.sh\""]).is_empty());
        let gone = verdicts(&m, &["'/h/Other Scripts/guard.sh'"]);
        assert_eq!(gone[0].1, "file not found: /h/Other Scripts/guard.sh");
    }

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
    #[test]
    fn leading_variable_assignments_are_skipped() {
        let mut m = Mock::default();
        m.script("/h/a.sh", true);
        assert!(verdicts(&m, &["RTOK_X=1 /h/a.sh"]).is_empty());
    }

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
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
        // Compared as paths: `join` writes `\` on Windows where the literals have `/`.
        let mut sources: Vec<PathBuf> = found.iter().map(|p| PathBuf::from(&p.source)).collect();
        sources.sort();
        assert_eq!(
            sources,
            vec![
                PathBuf::from("/h/.claude/settings.json"),
                PathBuf::from("/h/.claude/settings.local.json"),
                PathBuf::from("/proj/.claude/settings.json"),
                PathBuf::from("/proj/.claude/settings.local.json")
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

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
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
