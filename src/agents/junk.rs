// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok agents junk clear` (T182): remove junk rtok owns under its own home — log siblings
//! left behind past `[log] files` and archive payloads past `core.retain_calls_days`. With
//! `--yes` it also clears old hook bodies (`core.retain_hook_bodies_days`) and converts the
//! store to incremental vacuum if it is not already (T352; `mcp`/`proxy` do the same in the
//! background at session start). Dry run by default; `--yes` applies. `clear` never touches a
//! host directory. `list` (T330.1, T330.2) also shows every installed host's folders from
//! the `junk_map` of `research.md` §22, read-only, sized with a per-agent time limit.
//!
//! Inventory for T182 found nothing else unbounded in `~/.rtok`: log rotation (`src/log.rs`)
//! already caps generations on every write, archive retention (`Store::run_retention`) already
//! runs at proxy/MCP session start, and the semantic cache and graph index live inside
//! `rtok.db`, not as loose files — this command makes the first two runnable on demand and
//! reports what they would do before anyone applies them.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;

use super::junk_map::{Role, Roots, specs};
use super::{Agent, HOSTS, host, present};
use crate::config::Config;
use crate::info::human_bytes;
use crate::store::Store;

#[derive(Debug, Serialize)]
pub struct Outcome {
    /// `log` or `archive`.
    pub kind: &'static str,
    pub path: String,
    pub bytes: u64,
    /// `clear` or `keep` (only after `--yes` failed to remove it).
    pub action: &'static str,
    pub note: String,
    pub failed: bool,
}

/// `rtok.log.<N>` siblings beyond `[log] files`. `rotate()` (`src/log.rs`) only ever drops the
/// single generation past the *current* cap on each write, so lowering `files` after some ran
/// leaves the older generations behind forever — the one real unbounded case this inventory
/// found in rtok's own home.
fn stale_log_siblings(cfg: &Config) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (
        cfg.log.path.parent(),
        cfg.log.path.file_name().and_then(|n| n.to_str()),
    ) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let fname = e.file_name();
            let suffix = fname.to_str()?.strip_prefix(name)?.strip_prefix('.')?;
            (suffix.parse::<u32>().ok()? > cfg.log.files).then(|| e.path())
        })
        .collect()
}

fn outcome(kind: &'static str, path: PathBuf, note: String) -> Outcome {
    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Outcome {
        kind,
        path: path.display().to_string(),
        bytes,
        action: "clear",
        note,
        failed: false,
    }
}

/// Everything `clear` would touch. Read-only; an unreadable path is skipped, not fatal
/// (fail open).
pub fn scan(cfg: &Config) -> Vec<Outcome> {
    let mut out: Vec<Outcome> = stale_log_siblings(cfg)
        .into_iter()
        .map(|p| outcome("log", p, format!("past `[log] files` = {}", cfg.log.files)))
        .collect();
    if let Ok(store) = Store::open(&cfg.core.db_path)
        && let Ok(paths) = store.archives_pending_retention(cfg.core.retain_calls_days)
    {
        out.extend(paths.into_iter().map(|p| {
            outcome(
                "archive",
                p,
                format!(
                    "past `core.retain_calls_days` = {}",
                    cfg.core.retain_calls_days
                ),
            )
        }));
    }
    out
}

/// Dry run without `yes`. With it: removes the stray log siblings directly, then runs the
/// same store retention `proxy`/`mcp` already run at session start — same rows, same files,
/// just on demand. Never touches `rtok.db` itself or an archive still referenced by a call.
pub fn run(cfg: &Config, yes: bool) -> Vec<Outcome> {
    let mut outcomes = scan(cfg);
    if !yes {
        return outcomes;
    }
    for o in outcomes.iter_mut().filter(|o| o.kind == "log") {
        match std::fs::remove_file(&o.path) {
            Ok(()) => o.note = "removed".into(),
            Err(e) => {
                o.failed = true;
                o.action = "keep";
                o.note = format!("failed, kept: {e}");
            }
        }
    }
    if let Ok(store) = Store::open(&cfg.core.db_path) {
        // Fire-and-forget on disk like `run_retention` itself (T75/T182): a file it could not
        // remove is simply left for the next run, never fatal here.
        let _ = store.run_retention(cfg.core.retain_calls_days, cfg.core.retain_hook_bodies_days);
        // T352: the same one-time conversion `mcp`/`proxy` session start runs in the background,
        // here on demand (a no-op once the store is incremental).
        let _ = store.convert_to_incremental_vacuum();
        for o in outcomes.iter_mut().filter(|o| o.kind == "archive") {
            o.note = "removed".into();
        }
    }
    outcomes
}

pub fn to_table(outcomes: &[Outcome], yes: bool) -> String {
    let head = ["action", "kind", "path", "bytes"]
        .map(String::from)
        .to_vec();
    let mut lines = vec![head];
    lines.extend(outcomes.iter().map(|o| {
        vec![
            o.action.into(),
            o.kind.into(),
            o.path.clone(),
            human_bytes(o.bytes),
        ]
    }));
    let cols = [
        crate::render::Col::left(0),
        crate::render::Col::left(0),
        crate::render::Col::left(0),
        crate::render::Col::right(0),
    ];
    let notes = outcomes.iter().map(|o| o.note.as_str());
    let mut out = crate::worktree::noted_table(&cols, &lines, notes);
    let cleared = outcomes.iter().filter(|o| o.action == "clear" && !o.failed);
    let (n, bytes) = cleared.fold((0, 0), |(n, b), o| (n + 1, b + o.bytes));
    out.push_str(&match (yes, n) {
        (false, 0) => "\nnothing to clear\n".to_string(),
        (false, n) => format!(
            "\ndry run: {n} items, {} to free, nothing changed; rerun with --yes\n",
            human_bytes(bytes)
        ),
        (true, n) => format!("\nfreed {} in {n} items\n", human_bytes(bytes)),
    });
    out
}

/// One folder an agent writes to, with its disk usage.
#[derive(Debug, Serialize)]
pub struct Folder {
    pub path: String,
    /// `data`, `temp`, `logs` or `cache` (`junk_map::Role`); rtok's own folders are `data`.
    pub role: &'static str,
    pub size_bytes: u64,
    /// A row of `research.md` §22 backs this path (D36). Anything else is listed read-only:
    /// "not documented: not cleared".
    pub documented: bool,
    /// The other agents whose folder resolves to this same directory. It is sized once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shared_with: Vec<&'static str>,
    /// Why the size may be low or the path is not what it looks like: permission denied, scan
    /// timeout, or a symlink to the real folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One junk kind under an agent: what `clear` would remove, summed.
#[derive(Debug, Serialize)]
pub struct KindRow {
    pub kind: &'static str,
    /// `safe`: always regenerated, cleared by default (T330 classes).
    pub class: &'static str,
    pub items: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct AgentJunk {
    pub name: &'static str,
    /// A host from `agents::HOSTS`; `rtok` itself is the one row that is not.
    pub host: bool,
    pub installed: bool,
    pub folders: Vec<Folder>,
    /// Disk usage of the folders, a folder nested in another one counted once.
    pub total_bytes: u64,
    pub kinds: Vec<KindRow>,
    pub freed_default_bytes: u64,
}

/// `rtok agents junk list` (T330.1): per-agent folders, junk kinds and space `clear` frees.
#[derive(Debug, Serialize)]
pub struct Report {
    pub agents: Vec<AgentJunk>,
    /// Every folder once, however many agents share it.
    pub total_bytes: u64,
    pub freed_default_bytes: u64,
}

/// What one walk found. A folder it could not read or finish is a lower bound, never an error.
#[derive(Debug, Default, Clone, Copy)]
pub struct Usage {
    pub bytes: u64,
    pub denied: bool,
    pub timed_out: bool,
}

/// How long one agent's folders may take to size before the rest is reported as a lower bound
/// (a huge cache on a network volume must not hang `list`).
pub const AGENT_SCAN_LIMIT: Duration = Duration::from_secs(10);

/// Disk usage, not apparent size: allocated blocks on Unix so sparse files and APFS clones are
/// not over-counted, a hard link once. Elsewhere it is the apparent size with every link
/// counted, because std has no stable file id to dedupe by there. A symlink costs itself and is
/// never followed, so a link out of an agent folder cannot pull foreign bytes into the total.
pub fn disk_usage(path: &Path) -> u64 {
    disk_usage_until(path, None).bytes
}

/// [`disk_usage`] that stops at `deadline` and reports a directory it was not allowed to read.
pub fn disk_usage_until(path: &Path, deadline: Option<Instant>) -> Usage {
    fn walk(path: &Path, seen: &mut HashSet<(u64, u64)>, deadline: Option<Instant>, u: &mut Usage) {
        if u.timed_out || deadline.is_some_and(|d| Instant::now() >= d) {
            u.timed_out = true;
            return;
        }
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) => {
                u.denied |= e.kind() == std::io::ErrorKind::PermissionDenied;
                return;
            }
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() < 2 || seen.insert((meta.dev(), meta.ino())) {
                u.bytes += meta.blocks() * 512;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = &seen;
            u.bytes += meta.len();
        }
        if meta.is_dir() {
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        walk(&entry.path(), seen, deadline, u);
                    }
                }
                Err(e) => u.denied |= e.kind() == std::io::ErrorKind::PermissionDenied,
            }
        }
    }
    let mut usage = Usage::default();
    walk(path, &mut HashSet::new(), deadline, &mut usage);
    usage
}

/// The directories rtok's junk lives in: the log directory and the archive directory. One
/// nested inside the other is folded into the outer so a byte is shown once.
fn rtok_folders(cfg: &Config) -> Vec<Folder> {
    let mut dirs: Vec<PathBuf> = [cfg.log.path.parent(), Some(cfg.core.archive_dir.as_path())]
        .into_iter()
        .flatten()
        .filter(|d| d.is_dir())
        .map(Path::to_path_buf)
        .collect();
    dirs.sort();
    dirs.dedup();
    let all = dirs.clone();
    dirs.retain(|d| !all.iter().any(|o| o != d && d.starts_with(o)));
    dirs.into_iter()
        .map(|d| Folder {
            size_bytes: disk_usage(&d),
            path: d.display().to_string(),
            role: "data",
            documented: true,
            shared_with: Vec::new(),
            note: None,
        })
        .collect()
}

/// A folder a host writes, before it is sized.
struct Found {
    path: PathBuf,
    role: Role,
    documented: bool,
}

/// The folders of one host that exist: the §22 map, then the folders its own config files sit
/// in (a host with no §22 row, Cursor, still shows where its settings live, read-only). A
/// config-file folder inside a folder already found is dropped so a byte is not listed twice,
/// and the home directory itself is never a folder (Aider's files sit directly in it).
fn host_folders(a: &dyn Agent, cfg: &Config, roots: &Roots) -> Vec<Found> {
    let mut found: Vec<Found> = Vec::new();
    for s in specs(a.id()) {
        let path = roots.resolve(&s.path);
        let known = found.iter().any(|f| f.path == path);
        if !known && std::fs::symlink_metadata(&path).is_ok() {
            found.push(Found {
                path,
                role: s.role,
                documented: s.documented,
            });
        }
    }
    let mut parents: Vec<PathBuf> = a
        .variants()
        .iter()
        .flat_map(|v| a.markers(cfg, v.kind))
        .filter_map(|p| p.parent().map(Path::to_path_buf))
        .filter(|d| d.parent().is_some() && d != roots.home() && d.is_dir())
        .collect();
    parents.sort_by_key(|d| d.components().count());
    for d in parents {
        if !found.iter().any(|f| d.starts_with(&f.path)) {
            found.push(Found {
                path: d,
                role: Role::Data,
                documented: false,
            });
        }
    }
    found.sort_by(|x, y| x.path.cmp(&y.path));
    found
}

/// What the real path of `path` is, when it is a symlink: the folder is sized at its target
/// (the link itself costs nothing) and the listing says so.
fn symlink_target(path: &Path) -> Option<PathBuf> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    meta.file_type()
        .is_symlink()
        .then(|| std::fs::canonicalize(path).ok())
        .flatten()
}

/// Every host of `HOSTS` as an agent row: installed ones, or all of them with `all`. The
/// folders are sized once however many agents share them, each agent within `limit`.
fn host_rows(
    cfg: &Config,
    roots: &Roots,
    all: bool,
    limit: Duration,
) -> (Vec<AgentJunk>, BTreeMap<PathBuf, u64>) {
    let mut rows: Vec<AgentJunk> = Vec::new();
    let mut sized: HashMap<PathBuf, Usage> = HashMap::new();
    // Canonical folder -> the agents that own it, for the "shared with" note.
    let mut owners: HashMap<PathBuf, Vec<&'static str>> = HashMap::new();
    let mut keys: Vec<Vec<PathBuf>> = Vec::new();
    for &id in HOSTS {
        let Some(a) = host(id) else { continue };
        let installed = a.variants().iter().any(|v| present(a, v, cfg));
        if !installed && !all {
            continue;
        }
        let deadline = Instant::now() + limit;
        let mut folders = Vec::new();
        let mut agent_keys = Vec::new();
        for f in host_folders(a, cfg, roots) {
            let target = symlink_target(&f.path);
            let key = target.clone().unwrap_or_else(|| f.path.clone());
            let usage = *sized
                .entry(key.clone())
                .or_insert_with(|| disk_usage_until(&key, Some(deadline)));
            let mut notes = Vec::new();
            if let Some(t) = &target {
                notes.push(format!("symlink to {}", t.display()));
            }
            if usage.denied {
                notes.push("permission denied: size is a lower bound".into());
            }
            if usage.timed_out {
                notes.push(format!(
                    "scan stopped after {}s: size is a lower bound",
                    limit.as_secs()
                ));
            }
            owners.entry(key.clone()).or_default().push(a.id());
            agent_keys.push(key);
            folders.push(Folder {
                path: f.path.display().to_string(),
                role: f.role.as_str(),
                size_bytes: usage.bytes,
                documented: f.documented,
                shared_with: Vec::new(),
                note: (!notes.is_empty()).then(|| notes.join("; ")),
            });
        }
        keys.push(agent_keys);
        rows.push(AgentJunk {
            name: a.id(),
            host: true,
            installed,
            folders,
            total_bytes: 0,
            kinds: Vec::new(),
            freed_default_bytes: 0,
        });
    }
    for (row, agent_keys) in rows.iter_mut().zip(&keys) {
        for (folder, key) in row.folders.iter_mut().zip(agent_keys) {
            folder.shared_with = owners[key]
                .iter()
                .copied()
                .filter(|n| *n != row.name)
                .collect();
        }
        row.total_bytes = top_level_bytes(
            row.folders
                .iter()
                .zip(agent_keys)
                .map(|(f, k)| (k.as_path(), f.size_bytes)),
        );
    }
    let by_key = sized.into_iter().map(|(k, u)| (k, u.bytes)).collect();
    (rows, by_key)
}

/// The sum of the folders that are not inside another listed folder, so a nested log folder
/// is not added on top of its data home.
fn top_level_bytes<'a>(folders: impl Iterator<Item = (&'a Path, u64)> + Clone) -> u64 {
    let all: Vec<&Path> = folders.clone().map(|(p, _)| p).collect();
    folders
        .filter(|(p, _)| !all.iter().any(|o| o != p && p.starts_with(o)))
        .map(|(_, b)| b)
        .sum()
}

/// Which hosts a [`report`] covers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    /// Also hosts that are not installed (their rows have no folders unless one is left over).
    pub all: bool,
}

/// Read-only: the same [`scan`] `clear` runs, summed per kind for rtok, plus every installed
/// host's folders (T330.2). Nothing a host writes is cleared yet (T330.3).
pub fn report(cfg: &Config) -> Report {
    report_with(
        cfg,
        &Roots::from_env(),
        Options::default(),
        AGENT_SCAN_LIMIT,
    )
}

pub fn report_with(cfg: &Config, roots: &Roots, opts: Options, limit: Duration) -> Report {
    let outcomes = scan(cfg);
    let kinds: Vec<KindRow> = ["log", "archive"]
        .into_iter()
        .filter_map(|kind| {
            let rows = outcomes.iter().filter(|o| o.kind == kind);
            let (items, size_bytes) = rows.fold((0, 0), |(n, b), o| (n + 1, b + o.bytes));
            (items > 0).then_some(KindRow {
                kind,
                class: "safe",
                items,
                size_bytes,
            })
        })
        .collect();
    let freed = kinds.iter().map(|k| k.size_bytes).sum();
    let rtok_folders = rtok_folders(cfg);
    let rtok_total = rtok_folders.iter().map(|f| f.size_bytes).sum::<u64>();
    let mut agents = vec![AgentJunk {
        name: "rtok",
        host: false,
        installed: true,
        folders: rtok_folders,
        total_bytes: rtok_total,
        kinds,
        freed_default_bytes: freed,
    }];
    let (hosts, mut by_key) = host_rows(cfg, roots, opts.all, limit);
    agents.extend(hosts);
    for f in &agents[0].folders {
        by_key.insert(PathBuf::from(&f.path), f.size_bytes);
    }
    // Each directory once, however many agents list it, and a nested one inside its parent.
    let total_bytes = top_level_bytes(by_key.iter().map(|(p, b)| (p.as_path(), *b)));
    Report {
        agents,
        total_bytes,
        freed_default_bytes: freed,
    }
}

/// `text` as an OSC 8 hyperlink to the folder, for a terminal that renders it. A path that is
/// not absolute has no `file://` URL and stays plain.
fn folder_link(path: &str, text: &str) -> String {
    match url::Url::from_file_path(path) {
        Ok(u) => format!("\x1b]8;;{u}\x1b\\{text}\x1b]8;;\x1b\\"),
        Err(()) => text.to_string(),
    }
}

/// Text of `agents junk list` and of the Hosts page's junk section; `exact` prints raw bytes,
/// `links` wraps each folder path in an OSC 8 link (only for a terminal).
pub fn to_list(report: &Report, exact: bool, links: bool) -> String {
    let size = |n: u64| if exact { n.to_string() } else { human_bytes(n) };
    let mut out = String::new();
    for a in &report.agents {
        out.push_str(a.name);
        if a.host && !a.installed {
            out.push_str(" (not installed)");
        }
        out.push('\n');
        for f in &a.folders {
            let path = if links {
                folder_link(&f.path, &f.path)
            } else {
                f.path.clone()
            };
            out.push_str(&format!("  {path}  {}  {}", f.role, size(f.size_bytes)));
            if !f.documented {
                out.push_str("  not documented: not cleared");
            }
            if !f.shared_with.is_empty() {
                out.push_str(&format!("  shared with {}", f.shared_with.join(", ")));
            }
            if let Some(note) = &f.note {
                out.push_str(&format!("  ({note})"));
            }
            out.push('\n');
        }
        if a.host && a.folders.is_empty() {
            out.push_str("  no folders found\n");
        }
        for k in &a.kinds {
            out.push_str(&format!(
                "    {} ({}): {} items, {}\n",
                k.kind,
                k.class,
                k.items,
                size(k.size_bytes)
            ));
        }
        if !a.host || !a.kinds.is_empty() {
            out.push_str(&format!(
                "  Freed by `clear`: {}\n",
                size(a.freed_default_bytes)
            ));
        }
    }
    out.push_str(&format!(
        "total\n  Folders: {}\n  Freed by `clear`: {}\n",
        size(report.total_bytes),
        size(report.freed_default_bytes)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_dir(cfg: &Config) -> std::path::PathBuf {
        let dir = cfg.log.path.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stale_log_siblings_are_found_past_the_cap_and_current_ones_are_not() {
        let (mut cfg, _dir) = crate::testutil::config("junk-log-siblings");
        cfg.log.files = 2;
        let logs = log_dir(&cfg);
        std::fs::write(logs.join("rtok.log.1"), b"a").unwrap();
        std::fs::write(logs.join("rtok.log.2"), b"bb").unwrap();
        std::fs::write(logs.join("rtok.log.6"), b"stale").unwrap();
        assert_eq!(stale_log_siblings(&cfg), vec![logs.join("rtok.log.6")]);
    }

    #[test]
    fn dry_run_lists_without_deleting_and_yes_removes() {
        let (mut cfg, _dir) = crate::testutil::config("junk-dry-run");
        cfg.log.files = 1;
        let logs = log_dir(&cfg);
        let stale = logs.join("rtok.log.5");
        std::fs::write(&stale, b"junk").unwrap();

        let preview = run(&cfg, false);
        assert_eq!(preview.len(), 1);
        assert!(stale.exists(), "dry run must not delete anything");

        let applied = run(&cfg, true);
        assert_eq!(applied[0].note, "removed");
        assert!(!stale.exists());
    }

    #[test]
    fn no_db_at_the_configured_path_is_skipped_not_fatal() {
        let (cfg, _dir) = crate::testutil::config("junk-no-db");
        assert_eq!(scan(&cfg).len(), 0);
    }

    /// The fixture T182's card asks for: a call the retention window still covers (its
    /// archive must survive `clear`) and one old enough to purge (its archive is the one
    /// `clear` removes). Mirrors `Store::run_retention_purges_old_call_and_archive`.
    #[test]
    fn archive_retention_keeps_the_referenced_one_and_clears_the_orphan() {
        let (cfg, _dir) = crate::testutil::config("junk-archive-retention");
        let store = crate::store::Store::open(&cfg.core.db_path).unwrap();
        store
            .upsert_session("s1", Some(1), None, None, Some("proxy"))
            .unwrap();
        let spill = |byte: u8, ts: Option<i64>| {
            let call = store
                .insert_call(
                    "s1",
                    "proxy",
                    "api_request",
                    Some(1),
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
            // Distinct bodies: archives are content-addressed, so two identical bodies would
            // dedupe into one archive still "referenced" by the live call.
            let body = vec![byte; 70 * 1024];
            store
                .insert_call_io(
                    call,
                    Some(&body),
                    None,
                    64 * 1024,
                    Some(&cfg.core.archive_dir),
                )
                .unwrap();
            if let Some(ts) = ts {
                store.set_call_ts(call, ts).unwrap();
            }
        };
        spill(b'a', None); // live: stays inside `core.retain_calls_days`
        spill(b'b', Some(0)); // old enough to purge

        let outcomes = run(&cfg, true);
        let archives: Vec<_> = outcomes.iter().filter(|o| o.kind == "archive").collect();
        assert_eq!(archives.len(), 1, "only the orphan is reported");
        assert_eq!(archives[0].note, "removed");

        let left: Vec<_> = std::fs::read_dir(&cfg.core.archive_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(left.len(), 1, "the referenced archive survives");
    }

    /// Unix only: elsewhere `disk_usage` has no file id and counts each hard link.
    #[cfg(unix)]
    #[test]
    fn disk_usage_counts_a_hard_link_once_and_never_follows_a_symlink() {
        let (_cfg, dir) = crate::testutil::config("junk-disk-usage");
        let root = dir.join("agent");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.join("outside.bin");
        std::fs::write(&outside, vec![1u8; 64 * 1024]).unwrap();
        let a = root.join("a.bin");
        std::fs::write(&a, vec![2u8; 8 * 1024]).unwrap();
        let alone = disk_usage(&root);
        std::fs::hard_link(&a, root.join("b.bin")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        let with_links = disk_usage(&root);
        assert!(
            alone >= 8 * 1024,
            "allocated blocks cover the file: {alone}"
        );
        // The extra entries cost directory/inode metadata at most, never another 8 KB or the
        // 64 KB the link points at.
        assert!(with_links < alone + 8 * 1024, "{alone} -> {with_links}");
    }

    #[test]
    fn report_sums_the_kinds_the_clear_plan_lists_and_the_text_prints_them() {
        let (mut cfg, _dir) = crate::testutil::config("junk-report");
        cfg.log.files = 1;
        let logs = log_dir(&cfg);
        std::fs::write(logs.join("rtok.log.4"), b"1234").unwrap();
        std::fs::write(logs.join("rtok.log.5"), b"12345678").unwrap();

        let report = report(&cfg);
        let planned: u64 = scan(&cfg).iter().map(|o| o.bytes).sum();
        assert_eq!(planned, 12);
        assert_eq!(report.freed_default_bytes, planned);
        let rtok = &report.agents[0];
        assert_eq!(rtok.name, "rtok");
        assert_eq!((rtok.kinds[0].kind, rtok.kinds[0].items), ("log", 2));
        assert!(
            rtok.folders
                .iter()
                .any(|f| f.path == logs.display().to_string())
        );

        let text = to_list(&report, true, false);
        assert!(text.contains("log (safe): 2 items, 12"), "{text}");
        assert!(text.contains("Freed by `clear`: 12"), "{text}");
    }
    fn write(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    /// Roots over a fixture home with `env` as the only environment.
    fn roots(home: &Path, env: &[(&str, PathBuf)]) -> Roots {
        let env: Vec<(String, PathBuf)> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
        Roots::new(home.to_path_buf(), move |k| {
            env.iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone().into())
        })
    }

    fn agent<'a>(r: &'a Report, name: &str) -> &'a AgentJunk {
        r.agents
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("no `{name}` row"))
    }

    fn folder<'a>(a: &'a AgentJunk, path: &Path) -> &'a Folder {
        let want = path.display().to_string();
        a.folders
            .iter()
            .find(|f| f.path == want)
            .unwrap_or_else(|| panic!("`{}` has no folder {want}: {:?}", a.name, a.folders))
    }

    #[cfg(unix)]
    /// T330.2: Claude Code, Cursor and Codex under a fixture home. §22's rows are documented,
    /// Cursor's folders are not, and a symlink out of a folder is not followed.
    #[test]
    fn host_rows_list_their_folders_with_sizes_and_the_documented_mark() {
        let (cfg, dir) = crate::testutil::config("junk-hosts");
        let outside = dir.join("outside");
        write(&outside.join("big"), 1 << 20);
        let claude = dir.join(".claude");
        write(&claude.join("settings.json"), 10);
        write(&claude.join("debug/a.log"), 100);
        std::os::unix::fs::symlink(&outside, claude.join("link")).unwrap();
        write(&dir.join(".cursor/hooks.json"), 10);
        let cursor_data = dir.join("Library/Application Support/Cursor");
        write(&cursor_data.join("state"), 10);
        write(&dir.join(".codex/log/codex.log"), 10);
        write(&dir.join(".codex/config.toml"), 10);

        let report = report_with(
            &cfg,
            &roots(&dir, &[]),
            Options::default(),
            AGENT_SCAN_LIMIT,
        );

        let c = agent(&report, "claude");
        let home = folder(c, &claude);
        assert_eq!((home.role, home.documented), ("data", true));
        assert_eq!(home.size_bytes, disk_usage(&claude));
        assert!(
            home.size_bytes < 512 * 1024,
            "the link out of the folder was followed: {}",
            home.size_bytes
        );
        let logs = folder(c, &claude.join("debug"));
        assert_eq!((logs.role, logs.documented), ("logs", true));
        assert_eq!(logs.size_bytes, disk_usage(&claude.join("debug")));
        // The debug log is inside the data home, so the agent total does not add it twice.
        assert_eq!(c.total_bytes, home.size_bytes);

        let cur = agent(&report, "cursor");
        assert!(cur.folders.len() >= 2, "{:?}", cur.folders);
        assert!(
            cur.folders.iter().all(|f| !f.documented),
            "{:?}",
            cur.folders
        );
        let app = folder(cur, &cursor_data);
        assert_eq!(app.size_bytes, disk_usage(&cursor_data));
        assert!(folder(cur, &dir.join(".cursor")).size_bytes > 0);

        let x = agent(&report, "codex");
        assert_eq!(folder(x, &dir.join(".codex/log")).role, "logs");
        assert!(folder(x, &dir.join(".codex")).documented);

        let text = to_list(&report, true, false);
        assert!(
            text.contains(&format!(
                "{}  data  {}  not documented: not cleared",
                cursor_data.display(),
                app.size_bytes
            )),
            "{text}"
        );
        assert!(!text.contains("\x1b]8"), "no link off a terminal: {text}");
        let linked = to_list(&report, true, true);
        assert!(linked.contains("\x1b]8;;file://"), "{linked}");
    }

    /// An override that points two hosts at one directory: it is listed under both with a
    /// "shared with" note and counted once in the report total.
    #[test]
    fn a_folder_two_agents_share_is_sized_once_and_marked_shared() {
        let (mut cfg, dir) = crate::testutil::config("junk-shared");
        // rtok's own folders must not be the fixture home, which holds the shared folder.
        cfg.log.path = dir.join("none/rtok.log");
        cfg.core.archive_dir = dir.join("none/archive");
        let shared = dir.join("shared");
        write(&shared.join("blob"), 64 * 1024);
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::create_dir_all(dir.join(".codex")).unwrap();
        let env = [
            ("CLAUDE_CONFIG_DIR", shared.clone()),
            ("CODEX_HOME", shared.clone()),
        ];

        let report = report_with(
            &cfg,
            &roots(&dir, &env),
            Options::default(),
            AGENT_SCAN_LIMIT,
        );

        let c = folder(agent(&report, "claude"), &shared);
        let x = folder(agent(&report, "codex"), &shared);
        assert_eq!(c.shared_with, vec!["codex"]);
        assert_eq!(x.shared_with, vec!["claude"]);
        assert_eq!(c.size_bytes, x.size_bytes);
        let both = agent(&report, "claude").total_bytes + agent(&report, "codex").total_bytes;
        assert!(
            report.total_bytes < both,
            "{} !< {both}",
            report.total_bytes
        );
        assert!(to_list(&report, false, false).contains("shared with codex"));
    }

    #[test]
    fn all_adds_hosts_that_are_not_installed_and_the_default_skips_them() {
        let (cfg, dir) = crate::testutil::config("junk-all");
        let r = roots(&dir, &[]);
        let names = |report: &Report| report.agents.len();

        let some = report_with(&cfg, &r, Options::default(), AGENT_SCAN_LIMIT);
        let every = report_with(&cfg, &r, Options { all: true }, AGENT_SCAN_LIMIT);

        assert_eq!(every.agents.len(), HOSTS.len() + 1, "rtok plus every host");
        assert!(names(&some) < names(&every));
        let absent = every
            .agents
            .iter()
            .find(|a| a.host && !a.installed)
            .unwrap();
        assert!(absent.folders.is_empty());
        assert!(
            to_list(&every, false, false).contains(&format!("{} (not installed)", absent.name))
        );
    }

    #[test]
    fn a_scan_past_its_limit_is_reported_not_fatal() {
        let (cfg, dir) = crate::testutil::config("junk-timeout");
        write(&dir.join(".claude/settings.json"), 10);

        let report = report_with(&cfg, &roots(&dir, &[]), Options::default(), Duration::ZERO);

        let note = folder(agent(&report, "claude"), &dir.join(".claude"))
            .note
            .clone();
        assert!(note.is_some_and(|n| n.contains("scan stopped")));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_folder_is_reported_not_fatal() {
        use std::os::unix::fs::PermissionsExt;
        let (_cfg, dir) = crate::testutil::config("junk-denied");
        let locked = dir.join("locked");
        write(&locked.join("f"), 10);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let usage = disk_usage_until(&locked, None);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A superuser reads it anyway, which is not the case under test.
        if !usage.denied {
            assert!(usage.bytes > 0);
        }
        assert!(usage.denied || usage.bytes > 4096);
    }
}
