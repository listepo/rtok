// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok doctor --fix` for broken hooks (T331.5): the first doctor code that writes a host
//! config. It removes only the entries the check classed `broken-hook` and may fix, through the
//! JSONC editor `agents install` uses (so every other byte of the file stays), after a backup
//! and with an atomic swap. Without `--yes` it only prints the diff it would apply. A file that
//! changed since the check, or whose edit would change anything but the broken entries, is
//! skipped and reported, never written. A plugin's files, rtok's own hook entries and every
//! hook that is not plainly broken are never removed.

use std::path::{Path, PathBuf};

use super::hooks::{self, Probes, Problem, entries};
use super::probe::Writer;
use crate::agents::jsonc::{self, Seg};
use crate::config::Config;

/// What happened to one file.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Dry run: the diff is what `--yes` would write.
    Planned,
    Written,
    /// Left as it was, with the reason (changed since the check, the edit check failed).
    Skipped(String),
    /// The backup or the write failed; the file is as it was.
    Failed(String),
}

#[derive(Debug)]
pub struct FileFix {
    pub source: PathBuf,
    pub removed: Vec<Problem>,
    pub diff: String,
    pub backup: Option<PathBuf>,
    pub outcome: Outcome,
}

#[derive(Debug, Default)]
pub struct FixReport {
    pub files: Vec<FileFix>,
    /// Broken hooks this command will not remove, with why.
    pub refused: Vec<(Problem, &'static str)>,
    /// `broken-hook` findings left after the writes (the re-check).
    pub broken_left: usize,
}

impl FixReport {
    /// 1 when a selected hook was not fixed (skipped or failed), else 0.
    pub fn exit_code(&self) -> i32 {
        i32::from(
            self.files
                .iter()
                .any(|f| matches!(f.outcome, Outcome::Skipped(_) | Outcome::Failed(_))),
        )
    }
}

/// rtok's own hook entries are installed, updated and removed by `rtok agents`, which knows
/// the plugin rules (T275); a doctor removal would be undone by the next update.
fn is_rtok_own(command: &str) -> bool {
    let words = shlex::split(command).unwrap_or_default();
    words.iter().any(|w| {
        crate::agents::is_rtok_bin(w) || Path::new(w).file_stem().is_some_and(|s| s == "rtok-hook")
    })
}

/// `(event, group, hook)` of a `hooks.<event>[g]` or `hooks.<event>[g].hooks[h]` key path.
fn parse_path(path: &str) -> Option<(&str, usize, Option<usize>)> {
    let rest = path.strip_prefix("hooks.")?;
    let (event, rest) = rest.split_once('[')?;
    let (g, rest) = rest.split_once(']')?;
    let hook = match rest {
        "" => None,
        r => {
            let h = r.strip_prefix(".hooks[")?.strip_suffix(']')?;
            Some(h.parse().ok()?)
        }
    };
    Some((event, g.parse().ok()?, hook))
}

/// `raw` without the entries at `paths`, each followed by the removal of the group and the
/// event key it leaves empty. Removal runs from the last entry to the first so earlier indices
/// stay valid. `None` when any path leads nowhere.
fn remove_entries(raw: &str, file: &Path, paths: &[&str]) -> Option<String> {
    let mut targets: Vec<(&str, usize, Option<usize>)> =
        paths.iter().map(|p| parse_path(p)).collect::<Option<_>>()?;
    targets.sort_by(|a, b| b.cmp(a));
    let mut body = raw.to_string();
    for (event, g, hook) in targets {
        let group = [Seg::Key("hooks"), Seg::Key(event), Seg::Index(g)];
        let inner = [group[0], group[1], group[2], Seg::Key("hooks")];
        let target: Vec<Seg> = match hook {
            Some(h) => inner.iter().copied().chain([Seg::Index(h)]).collect(),
            None => group.to_vec(),
        };
        let (next, found) = jsonc::remove_at(&body, file, &target).ok()?;
        if !found {
            return None;
        }
        body = next;
        if hook.is_some() && jsonc::is_empty_at(&body, &inner) {
            body = jsonc::remove_at(&body, file, &group).ok()?.0;
        }
        let events = [Seg::Key("hooks"), Seg::Key(event)];
        if jsonc::is_empty_at(&body, &events) {
            body = jsonc::remove_at(&body, file, &events).ok()?.0;
        }
    }
    Some(body)
}

/// The `(event, matcher, command)` of every hook of `raw`, in order.
fn hook_list(raw: &str) -> Option<Vec<(String, Option<String>, String)>> {
    let doc = jsonc::parse(raw).ok()?;
    Some(
        entries(&doc)
            .into_iter()
            .map(|e| (e.event, e.matcher, e.command))
            .collect(),
    )
}

/// The edit of `raw` that drops `removed`, or the reason it cannot be trusted: the new text must
/// still parse and hold exactly the old hooks minus the removed ones.
fn edit(raw: &str, file: &Path, removed: &[&Problem]) -> Result<String, &'static str> {
    let paths: Vec<&str> = removed.iter().map(|p| p.path.as_str()).collect();
    let body = remove_entries(raw, file, &paths).ok_or("an entry could not be located")?;
    let (before, after) = (hook_list(raw), hook_list(&body));
    let expect: Option<Vec<_>> = before.map(|all| {
        let mut gone: Vec<_> = removed
            .iter()
            .map(|p| (p.event.clone(), p.matcher.clone(), p.command.clone()))
            .collect();
        all.into_iter()
            .filter(|h| match gone.iter().position(|g| g == h) {
                Some(i) => {
                    gone.swap_remove(i);
                    false
                }
                None => true,
            })
            .collect()
    });
    if after.is_none() || after != expect {
        return Err("the edit would change more than the broken hooks");
    }
    Ok(body)
}

/// Remove every fixable broken hook. `apply` false only plans. `keep` is how many backup
/// generations to keep per file.
pub fn fix_broken(cfg: &Config, p: &Probes, w: &dyn Writer, apply: bool, keep: usize) -> FixReport {
    fix_broken_for(cfg, p, w, apply, keep, None)
}

/// [`fix_broken`] for one host's hooks (`--agent`); `None` is every host.
pub fn fix_broken_for(
    cfg: &Config,
    p: &Probes,
    w: &dyn Writer,
    apply: bool,
    keep: usize,
    agent: Option<&str>,
) -> FixReport {
    let mut report = FixReport::default();
    let mut by_file: Vec<(String, Vec<Problem>)> = Vec::new();
    for problem in hooks::check_for(cfg, p, agent) {
        if problem.kind != "broken-hook" {
            continue;
        }
        if !problem.fixable {
            let why = if problem.source.ends_with(".toml") {
                "TOML hook files are not edited yet"
            } else {
                "not a file of yours to edit"
            };
            report.refused.push((problem, why));
        } else if is_rtok_own(&problem.command) {
            report.refused.push((
                problem,
                "rtok's own entry: `rtok agents install` rewrites it",
            ));
        } else if let Some((_, list)) = by_file.iter_mut().find(|(s, _)| *s == problem.source) {
            list.push(problem);
        } else {
            by_file.push((problem.source.clone(), vec![problem]));
        }
    }
    for (source, problems) in by_file {
        let path = PathBuf::from(&source);
        report
            .files
            .push(fix_file(p, w, &path, problems, apply, keep));
    }
    if apply {
        report.broken_left = hooks::check_for(cfg, p, agent)
            .iter()
            .filter(|x| x.kind == "broken-hook")
            .count();
    }
    report
}

fn fix_file(
    p: &Probes,
    w: &dyn Writer,
    path: &Path,
    removed: Vec<Problem>,
    apply: bool,
    keep: usize,
) -> FileFix {
    let mut fix = FileFix {
        source: path.to_path_buf(),
        removed,
        diff: String::new(),
        backup: None,
        outcome: Outcome::Planned,
    };
    let refs: Vec<&Problem> = fix.removed.iter().collect();
    let planned =
        p.fs.read(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))
            .and_then(|raw| {
                let body = edit(&raw, path, &refs).map_err(str::to_string)?;
                Ok((raw, body))
            });
    let (raw, body) = match planned {
        Ok(ok) => ok,
        Err(why) => {
            fix.outcome = Outcome::Skipped(why);
            return fix;
        }
    };
    fix.diff = crate::render::unified_diff(path, &raw, &body);
    if !apply {
        return fix;
    }
    // The window between the check and this write is short but real: an agent or an editor may
    // have saved the file, and its change must not be overwritten with a copy of the old text.
    if p.fs.read(path).ok().as_deref() != Some(raw.as_str()) {
        fix.outcome = Outcome::Skipped("changed since the check".into());
        return fix;
    }
    match w.backup(path, keep) {
        Ok(bak) => fix.backup = bak,
        Err(e) => {
            fix.outcome = Outcome::Failed(format!("backup failed: {e}"));
            return fix;
        }
    }
    fix.outcome = match w.write(path, &body) {
        Ok(()) => Outcome::Written,
        Err(e) => Outcome::Failed(format!("write failed: {e}")),
    };
    fix
}

/// The text `rtok doctor --fix` prints.
pub fn render(r: &FixReport, apply: bool) -> String {
    let mut out = String::new();
    if r.files.is_empty() && r.refused.is_empty() {
        return "no broken hooks to remove\n".into();
    }
    if !apply {
        out.push_str("dry run: nothing is written. Add --yes to remove the hooks below.\n");
    }
    for f in &r.files {
        out.push_str(&format!("{}\n", f.source.display()));
        for p in &f.removed {
            let matcher = p
                .matcher
                .as_deref()
                .map(|m| format!("[{m}]"))
                .unwrap_or_default();
            let verb = if apply { "remove" } else { "would remove" };
            out.push_str(&format!(
                "  {verb} {}{matcher} `{}`: {}\n",
                p.event, p.command, p.detail
            ));
        }
        if let Some(b) = &f.backup {
            out.push_str(&format!("  backup {}\n", b.display()));
        }
        match &f.outcome {
            Outcome::Planned => out.push_str(&f.diff),
            Outcome::Written => out.push_str("  written\n"),
            Outcome::Skipped(why) => out.push_str(&format!("  skipped: {why}\n")),
            Outcome::Failed(why) => out.push_str(&format!("  failed: {why}\n")),
        }
    }
    for (p, why) in &r.refused {
        out.push_str(&format!(
            "not removed: {} `{}` in {}: {why}\n",
            p.event, p.command, p.source
        ));
    }
    if apply {
        let done: usize = r
            .files
            .iter()
            .filter(|f| f.outcome == Outcome::Written)
            .map(|f| f.removed.len())
            .sum();
        out.push_str(&format!(
            "{done} broken hook(s) removed, {} left\n",
            r.broken_left
        ));
    }
    out
}

/// `--fix` against this machine: the report text and the exit code.
pub fn run(cfg: &Config, apply: bool, agent: Option<&str>) -> (String, i32) {
    let probes = Probes {
        fs: &super::probe::RealFs,
        env: &super::probe::RealEnv,
        which: &super::probe::RealWhich,
    };
    let r = fix_broken_for(
        cfg,
        &probes,
        &super::probe::RealWriter,
        apply,
        cfg.setup.backup_files as usize,
        agent,
    );
    (render(&r, apply), r.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::probe::{Env, Fs, PathKind, Which};
    use proptest::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::io;

    /// An in-memory machine whose files the `Writer` side can change.
    #[derive(Default)]
    struct Machine {
        files: RefCell<BTreeMap<PathBuf, String>>,
        fail_backup: bool,
        fail_write: bool,
        /// Rewrites the file when it is read for the `race_at`-th time (an editor saving in
        /// between the plan and the write).
        race: RefCell<Option<(PathBuf, String)>>,
        race_at: Cell<usize>,
        reads: RefCell<usize>,
        backups: RefCell<Vec<PathBuf>>,
    }

    impl Fs for Machine {
        fn canonical(&self, path: &Path) -> PathBuf {
            path.to_path_buf()
        }
        fn read(&self, path: &Path) -> io::Result<String> {
            *self.reads.borrow_mut() += 1;
            let hit = self.race.borrow().clone();
            if let Some((p, body)) = hit
                && p == path
                && *self.reads.borrow() == self.race_at.get()
            {
                self.files.borrow_mut().insert(p, body);
            }
            self.files
                .borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
        fn kind(&self, path: &Path) -> PathKind {
            if self.files.borrow().contains_key(path) {
                PathKind::File { executable: true }
            } else {
                PathKind::Missing
            }
        }
    }
    impl Env for Machine {
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
    impl Which for Machine {
        fn find(&self, _: &str) -> Option<PathBuf> {
            None
        }
    }
    impl Writer for Machine {
        fn backup(&self, path: &Path, _keep: usize) -> io::Result<Option<PathBuf>> {
            if self.fail_backup {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            let bak = PathBuf::from(format!("/h/_backup/{}.bak", path.display()));
            self.backups.borrow_mut().push(bak.clone());
            Ok(Some(bak))
        }
        fn write(&self, path: &Path, body: &str) -> io::Result<()> {
            if self.fail_write {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied));
            }
            self.files.borrow_mut().insert(path.into(), body.into());
            Ok(())
        }
    }

    const SETTINGS: &str = "/h/.claude/settings.json";

    fn cfg() -> Config {
        let mut c = Config::default();
        c.doctor.settings_path = SETTINGS.into();
        c.setup.claude.settings_path = SETTINGS.into();
        c.setup.cursor.hooks_path = "/h/.cursor/hooks.json".into();
        c.setup.gemini.dir = "/h/.gemini".into();
        c
    }

    fn machine(settings: &str) -> Machine {
        let m = Machine::default();
        m.files
            .borrow_mut()
            .insert(SETTINGS.into(), settings.into());
        m
    }

    fn fix(m: &Machine, apply: bool) -> FixReport {
        let probes = Probes {
            fs: m,
            env: m,
            which: m,
        };
        fix_broken(&cfg(), &probes, m, apply, 3)
    }

    const TWO_HOOKS: &str = r#"{
  // keep this comment
  "theme": "dark",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "/h/gone/old.sh" },
          { "type": "command", "command": "/h/.claude/settings.json" }
        ]
      }
    ]
  }
}
"#;

    #[cfg(unix)] // POSIX paths and command words; Windows rules are T331.9
    #[test]
    fn a_dry_run_writes_nothing_and_shows_the_diff() {
        let m = machine(TWO_HOOKS);
        let r = fix(&m, false);
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.files[0].outcome, Outcome::Planned);
        assert!(
            r.files[0]
                .diff
                .contains("-          { \"type\": \"command\", \"command\": \"/h/gone/old.sh\" },")
        );
        assert_eq!(m.files.borrow()[Path::new(SETTINGS)], TWO_HOOKS);
        assert!(m.backups.borrow().is_empty());
        assert_eq!(r.exit_code(), 0);
        let text = render(&r, false);
        assert!(text.starts_with("dry run: nothing is written."), "{text}");
        assert!(text.contains("would remove PreToolUse[Bash]"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn apply_backs_up_first_then_removes_only_the_broken_entry() {
        let m = machine(TWO_HOOKS);
        let r = fix(&m, true);
        assert_eq!(r.files[0].outcome, Outcome::Written);
        assert_eq!(m.backups.borrow().len(), 1);
        assert_eq!(r.files[0].backup.as_ref(), m.backups.borrow().first());
        let after = m.files.borrow()[Path::new(SETTINGS)].clone();
        assert_eq!(
            after,
            TWO_HOOKS.replace(
                "{ \"type\": \"command\", \"command\": \"/h/gone/old.sh\" },",
                ""
            )
        );
        assert_eq!(r.broken_left, 0);
        assert!(render(&r, true).contains("1 broken hook(s) removed, 0 left"));
        // Idempotent: the second run has nothing to do.
        assert_eq!(render(&fix(&m, true), true), "no broken hooks to remove\n");
    }

    #[cfg(unix)]
    #[test]
    fn the_last_hook_takes_its_group_and_its_event_with_it() {
        let raw = r#"{
  "hooks": {
    "Stop": [ { "hooks": [ { "type": "command", "command": "/h/gone/a.sh" } ] } ],
    "PreToolUse": [ { "matcher": "Bash", "hooks": [ { "type": "command", "command": "/h/gone/b.sh" } ] } ]
  },
  "theme": "dark"
}
"#;
        let m = machine(raw);
        let r = fix(&m, true);
        assert_eq!(r.files[0].outcome, Outcome::Written);
        let after = m.files.borrow()[Path::new(SETTINGS)].clone();
        let doc = jsonc::parse(&after).unwrap();
        assert_eq!(doc["hooks"], serde_json::json!({}));
        assert_eq!(doc["theme"], "dark");
        assert_eq!(r.broken_left, 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_that_changed_since_the_check_is_skipped_and_untouched() {
        let m = machine(TWO_HOOKS);
        let edited = TWO_HOOKS.replace("\"dark\"", "\"light\"");
        let planned = fix(&m, false);
        assert_eq!(planned.files[0].outcome, Outcome::Planned);
        // The apply run repeats the dry run's reads and then re-reads once before the write.
        let dry_reads = m.reads.replace(0);
        m.race_at.set(dry_reads + 1);
        *m.race.borrow_mut() = Some((SETTINGS.into(), edited.clone()));
        let r = fix(&m, true);
        assert_eq!(
            r.files[0].outcome,
            Outcome::Skipped("changed since the check".into())
        );
        assert_eq!(r.exit_code(), 1);
        assert!(m.backups.borrow().is_empty());
        assert_eq!(m.files.borrow()[Path::new(SETTINGS)], edited);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_backup_or_write_leaves_the_file_and_exits_1() {
        let mut m = machine(TWO_HOOKS);
        m.fail_backup = true;
        let r = fix(&m, true);
        assert!(
            matches!(&r.files[0].outcome, Outcome::Failed(w) if w.starts_with("backup failed"))
        );
        assert_eq!(r.exit_code(), 1);
        assert_eq!(m.files.borrow()[Path::new(SETTINGS)], TWO_HOOKS);

        let mut m = machine(TWO_HOOKS);
        m.fail_write = true;
        let r = fix(&m, true);
        assert!(matches!(&r.files[0].outcome, Outcome::Failed(w) if w.starts_with("write failed")));
        assert_eq!(r.exit_code(), 1);
        assert_eq!(m.files.borrow()[Path::new(SETTINGS)], TWO_HOOKS);
    }

    #[cfg(unix)]
    #[test]
    fn rtok_own_entries_and_non_broken_hooks_are_never_removed() {
        let raw = r#"{"hooks":{"PreToolUse":[{"hooks":[
{"type":"command","command":"/h/gone/rtok hook pre-tool-use"},
{"type":"command","command":"/h/gone/rtok-hook"},
{"type":"command","command":"/h/.claude/settings.json"},
{"type":"command","command":"echo hi"}
]}]}}"#;
        let m = machine(raw);
        let r = fix(&m, true);
        assert!(r.files.is_empty(), "{r:?}");
        assert_eq!(r.refused.len(), 2);
        assert_eq!(m.files.borrow()[Path::new(SETTINGS)], raw);
        assert_eq!(r.exit_code(), 0);
        assert!(render(&r, true).contains("not removed:"));
    }

    #[test]
    fn is_rtok_own_matches_the_binary_and_the_hook_shim_only() {
        assert!(is_rtok_own("/usr/local/bin/rtok hook pre-tool-use"));
        assert!(is_rtok_own("'/x y/rtok-hook' run"));
        assert!(!is_rtok_own("/h/scripts/rtokish.sh"));
    }

    #[test]
    fn parse_path_reads_nested_and_flat_key_paths() {
        assert_eq!(
            parse_path("hooks.PreToolUse[1].hooks[2]"),
            Some(("PreToolUse", 1, Some(2)))
        );
        assert_eq!(parse_path("hooks.Stop[0]"), Some(("Stop", 0, None)));
        assert_eq!(parse_path("hooks.Stop"), None);
        assert_eq!(parse_path("other.Stop[0]"), None);
        assert_eq!(parse_path("hooks.Stop[x]"), None);
    }

    // ---- property: the bytes outside the removed entry are unchanged ----

    /// Filler placed between JSON tokens: whitespace and both comment forms.
    const FILLERS: [&str; 7] = ["", " ", "\n", "\n    ", "// note\n", "/* é — ü */", "\t"];

    #[derive(Debug, Clone)]
    struct Spec {
        /// Per event: per group: `Some(hooks)` nested with that many hooks, `None` flat.
        events: Vec<Vec<Option<usize>>>,
        fillers: Vec<usize>,
        pick: (usize, usize, usize),
    }

    fn spec() -> impl Strategy<Value = Spec> {
        (
            prop::collection::vec(
                prop::collection::vec(prop::option::of(1usize..4), 1..4),
                1..4,
            ),
            prop::collection::vec(0..FILLERS.len(), 64..65),
            (0usize..8, 0usize..8, 0usize..8),
        )
            .prop_map(|(events, fillers, pick)| Spec {
                events,
                fillers,
                pick,
            })
    }

    const EVENTS: [&str; 3] = ["PreToolUse", "PostToolUse", "Stop"];

    /// The document, the key path to remove, and the command that path holds.
    fn build(s: &Spec) -> (String, String, String) {
        let mut f = s.fillers.iter().cycle().map(|i| FILLERS[*i]);
        let mut next = move || f.next().unwrap();
        let (pe, pg, ph) = (
            s.pick.0 % s.events.len(),
            s.pick.1 % s.events[s.pick.0 % s.events.len()].len(),
            s.pick.2,
        );
        let mut path = String::new();
        let mut command = String::new();
        let mut out = format!(
            "{}{{{}\"theme\"{}:{}\"dark\",{}\"hooks\"{}:{}{{{}",
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
            next(),
            next()
        );
        for (e, groups) in s.events.iter().enumerate() {
            if e > 0 {
                out += &format!(",{}", next());
            }
            out += &format!("\"{}\"{}:{}[{}", EVENTS[e], next(), next(), next());
            for (g, group) in groups.iter().enumerate() {
                if g > 0 {
                    out += &format!(",{}", next());
                }
                let cmd = |h: usize| format!("/gone/{e}-{g}-{h}.sh");
                match group {
                    None => {
                        out += &format!(
                            "{{\"type\":{}\"command\",{}\"command\":\"{}\"}}",
                            next(),
                            next(),
                            cmd(0)
                        );
                        if (e, g) == (pe, pg) {
                            path = format!("hooks.{}[{g}]", EVENTS[e]);
                            command = cmd(0);
                        }
                    }
                    Some(n) => {
                        out += &format!(
                            "{{{}\"matcher\":{}\"Bash\",{}\"hooks\"{}:{}[{}",
                            next(),
                            next(),
                            next(),
                            next(),
                            next(),
                            next()
                        );
                        for h in 0..*n {
                            if h > 0 {
                                out += &format!(",{}", next());
                            }
                            out += &format!(
                                "{{\"type\":{}\"command\",\"command\":\"{}\"}}",
                                next(),
                                cmd(h)
                            );
                        }
                        out += &format!("{}]{}}}", next(), next());
                        if (e, g) == (pe, pg) {
                            let h = ph % n;
                            path = format!("hooks.{}[{g}].hooks[{h}]", EVENTS[e]);
                            command = cmd(h);
                        }
                    }
                }
            }
            out += &format!("{}]", next());
        }
        out += &format!("{}}}{}}}{}", next(), next(), next());
        (out, path, command)
    }

    /// `after` is `before` with one contiguous region removed.
    fn only_a_region_is_gone(before: &str, after: &str) -> bool {
        let lcp = before
            .bytes()
            .zip(after.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        before.len() >= after.len() && before.as_bytes().ends_with(&after.as_bytes()[lcp..])
    }

    proptest! {
        #[test]
        fn removing_one_hook_changes_only_that_hook(s in spec()) {
            let (raw, path, command) = build(&s);
            prop_assume!(!path.is_empty());
            let file = Path::new("settings.json");
            prop_assert!(jsonc::parse(&raw).is_ok(), "generator made invalid JSONC: {raw}");
            let body = remove_entries(&raw, file, &[path.as_str()]).expect("entry located");
            prop_assert!(only_a_region_is_gone(&raw, &body), "before: {raw}\nafter: {body}");
            let before = hook_list(&raw).unwrap();
            let after = hook_list(&body).expect("still parses");
            let mut expect = before.clone();
            let at = expect.iter().position(|h| h.2 == command).unwrap();
            expect.remove(at);
            prop_assert_eq!(after, expect);
            let quoted = format!("\"{command}\"");
            prop_assert!(!body.contains(&quoted));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_broken_toml_hook_is_reported_and_never_edited() {
        let toml = "[[hooks]]\nevent = \"Stop\"\ncommand = \"/h/gone.sh\"\n";
        let m = machine("{}");
        m.files
            .borrow_mut()
            .insert("/h/.kimi-code/config.toml".into(), toml.into());
        let mut c = cfg();
        c.setup.kimi.config_path = "/h/.kimi-code/config.toml".into();
        let probes = Probes {
            fs: &m,
            env: &m,
            which: &m,
        };
        let r = fix_broken(&c, &probes, &m, true, 3);
        assert!(r.files.is_empty(), "{r:?}");
        assert_eq!(r.refused.len(), 1);
        assert_eq!(r.refused[0].1, "TOML hook files are not edited yet");
        assert_eq!(
            m.files.borrow()[Path::new("/h/.kimi-code/config.toml")],
            toml
        );
        assert!(m.backups.borrow().is_empty());
    }
}
