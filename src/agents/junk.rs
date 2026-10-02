//! `rtok agents junk clear` (T182): remove junk rtok owns under its own home — log siblings
//! left behind past `[log] files` and archive payloads past `core.retain_calls_days`. With
//! `--yes` it also clears old hook bodies (`core.retain_hook_bodies_days`) and converts the
//! store to incremental vacuum if it is not already (T352; `mcp`/`proxy` do the same in the
//! background at session start). Dry run by default; `--yes` applies. Host junk
//! (each app's own scratch/cache folders) is out of scope until the per-host map lands (plan
//! T182 item 3) — this never touches a host directory.
//!
//! Inventory for T182 found nothing else unbounded in `~/.rtok`: log rotation (`src/log.rs`)
//! already caps generations on every write, archive retention (`Store::run_retention`) already
//! runs at proxy/MCP session start, and the semantic cache and graph index live inside
//! `rtok.db`, not as loose files — this command makes the first two runnable on demand and
//! reports what they would do before anyone applies them.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

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
    pub size_bytes: u64,
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
    pub folders: Vec<Folder>,
    pub kinds: Vec<KindRow>,
    pub freed_default_bytes: u64,
}

/// `rtok agents junk list` (T330.1): per-agent folders, junk kinds and space `clear` frees.
#[derive(Debug, Serialize)]
pub struct Report {
    pub agents: Vec<AgentJunk>,
    pub freed_default_bytes: u64,
}

/// Disk usage, not apparent size: allocated blocks on Unix so sparse files and APFS clones are
/// not over-counted, a hard link once. Elsewhere it is the apparent size with every link
/// counted, because std has no stable file id to dedupe by there. A symlink costs itself and is
/// never followed, so a link out of an agent folder cannot pull foreign bytes into the total.
pub fn disk_usage(path: &Path) -> u64 {
    fn walk(path: &Path, seen: &mut HashSet<(u64, u64)>) -> u64 {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return 0;
        };
        let mut total = 0;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() < 2 || seen.insert((meta.dev(), meta.ino())) {
                total += meta.blocks() * 512;
            }
        }
        #[cfg(not(unix))]
        {
            let _ = &seen;
            total += meta.len();
        }
        if meta.is_dir() {
            for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
                total += walk(&entry.path(), seen);
            }
        }
        total
    }
    walk(path, &mut HashSet::new())
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
        })
        .collect()
}

/// Read-only: the same [`scan`] `clear` runs, summed per kind. Only rtok's own junk exists
/// until the per-host map is wired (T330.2), so there is one agent row.
pub fn report(cfg: &Config) -> Report {
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
    Report {
        agents: vec![AgentJunk {
            name: "rtok",
            folders: rtok_folders(cfg),
            kinds,
            freed_default_bytes: freed,
        }],
        freed_default_bytes: freed,
    }
}

/// Text of `agents junk list` and of the Hosts page's junk section; `exact` prints raw bytes.
pub fn to_list(report: &Report, exact: bool) -> String {
    let size = |n: u64| if exact { n.to_string() } else { human_bytes(n) };
    let mut out = String::new();
    for a in &report.agents {
        out.push_str(&format!("{}\n", a.name));
        for f in &a.folders {
            out.push_str(&format!("  {}  {}\n", f.path, size(f.size_bytes)));
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
        out.push_str(&format!(
            "  Freed by `clear`: {}\n",
            size(a.freed_default_bytes)
        ));
    }
    out.push_str(&format!(
        "total\n  Freed by `clear`: {}\n",
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

        let text = to_list(&report, true);
        assert!(text.contains("log (safe): 2 items, 12"), "{text}");
        assert!(text.contains("Freed by `clear`: 12"), "{text}");
    }
}
