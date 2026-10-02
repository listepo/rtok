// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T150: the worktree inventory against a real repository — a squash-merged branch, a
//! dirty worktree locked by another owner, and a record whose directory was deleted.

use std::path::Path;
use std::process::Command;

use rtok::worktree::{Entry, State, git, inventory};

fn run(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .expect("git runs");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "git {args:?}: {err}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn commit(dir: &Path, file: &str) {
    std::fs::write(dir.join(file), file).unwrap();
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", file]);
}

fn find<'a>(entries: &'a [Entry], name: &str) -> &'a Entry {
    let found = entries.iter().find(|e| e.record.path.ends_with(name));
    found.unwrap_or_else(|| panic!("{name} missing in {entries:?}"))
}

#[test]
fn inventory_sees_a_squash_merge_a_foreign_lock_and_a_deleted_directory() {
    let tmp = rtok::testutil::tmp_dir("worktree");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);
    assert_eq!(git::default_base(&work), "origin/main");

    run(
        &work,
        &["worktree", "add", "-q", "-b", "t1-merged", "../wt-merged"],
    );
    commit(&tmp.join("wt-merged"), "b.txt");
    let reason = "Cursor / grok | t2 | 2026-09-22";
    let locked = ["worktree", "add", "-q", "--lock", "--reason", reason];
    run(
        &work,
        &[&locked[..], &["-b", "t2-open", "../wt-open"]].concat(),
    );
    commit(&tmp.join("wt-open"), "c.txt");
    std::fs::write(tmp.join("wt-open/untracked.txt"), "x").unwrap();
    run(
        &work,
        &[&locked[..], &["-b", "t3-gone", "../wt-gone"]].concat(),
    );
    std::fs::remove_dir_all(tmp.join("wt-gone")).unwrap();

    run(&work, &["merge", "-q", "--squash", "t1-merged"]);
    run(&work, &["commit", "-q", "-m", "squash t1"]);
    run(&work, &["push", "-q", "origin", "main"]);
    let merged = run(&work, &["branch", "--merged", "origin/main"]);
    assert!(
        !merged.contains("t1-merged"),
        "git cannot see a squash merge: {merged}"
    );

    let entries = inventory(&work).unwrap();
    let state = |name: &str| find(&entries, name);
    assert_eq!(state("work").state, State::Main);
    assert_eq!(state("wt-merged").state, State::Merged);
    assert!(!state("wt-merged").record.held_against(None));

    let open = state("wt-open");
    assert_eq!(open.state, State::Dirty);
    assert_eq!(open.record.branch.as_deref(), Some("t2-open"));
    assert_eq!(open.record.owner().unwrap().task, "t2");
    assert!(open.record.held_against(Some("Claude Code / sonnet")));
    assert!(!open.record.held_against(Some("Cursor / grok")));

    // Locked, so git never marks it prunable — the missing directory is the only signal.
    let gone = state("wt-gone");
    assert_eq!((gone.state, &gone.record.prunable), (State::Stale, &None));

    // A committed, unmerged branch in a clean worktree.
    std::fs::remove_file(tmp.join("wt-open/untracked.txt")).unwrap();
    assert_eq!(
        find(&inventory(&work).unwrap(), "wt-open").state,
        State::Unmerged
    );
}

fn rtok(cwd: &Path, args: &[&str]) -> std::process::Output {
    rtok_in(cwd, cwd, args, b"")
}

/// `rtok` with its store under `home`, fed `stdin`.
fn rtok_in(home: &Path, cwd: &Path, args: &[&str], stdin: &[u8]) -> std::process::Output {
    rtok_as(home, cwd, None, args, stdin)
}

/// [`rtok_in`] as the rtok agent `agent` (`RTOK_AGENT_ID`); `None` clears the caller's own.
fn rtok_as(
    home: &Path,
    cwd: &Path,
    agent: Option<&str>,
    args: &[&str],
    stdin: &[u8],
) -> std::process::Output {
    use std::io::Write as _;
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rtok"));
    match agent {
        Some(id) => cmd.env("RTOK_AGENT_ID", id),
        None => cmd.env_remove("RTOK_AGENT_ID"),
    };
    let mut child = cmd
        .current_dir(cwd)
        .env("HOME", home)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("rtok spawns");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().expect("rtok runs")
}

/// A successful `rtok … --json` run, parsed.
fn json(cwd: &Path, args: &[&str]) -> serde_json::Value {
    json_in(cwd, cwd, args)
}

fn json_in(home: &Path, cwd: &Path, args: &[&str]) -> serde_json::Value {
    let out = rtok_in(home, cwd, args, b"");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "rtok {args:?}: {err}");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn by_name<'a>(rows: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    let found = rows.as_array().unwrap().iter().find(|r| {
        let path = r["path"].as_str().or(r["worktree"].as_str()).unwrap();
        Path::new(path).ends_with(name)
    });
    found.unwrap_or_else(|| panic!("{name} missing in {rows}"))
}

/// T151: `rtok worktree list` splits tagged build cache from source and finds the
/// directory git no longer lists.
#[test]
fn list_splits_cache_from_source_and_reports_an_orphan() {
    let tmp = rtok::testutil::tmp_dir("worktree-list");
    run(&tmp, &["init", "-q", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    // Shared by every worktree of the repository, like a committed `.gitignore`.
    std::fs::write(work.join(".git/info/exclude"), "target/\nout/\n").unwrap();
    for name in ["cache", "dirty", "orphan"] {
        let (branch, path) = (format!("t-{name}"), format!("../wt-{name}"));
        run(&work, &["worktree", "add", "-q", "-b", &branch, &path]);
    }
    let cache = tmp.join("wt-cache");
    std::fs::create_dir_all(cache.join("target/debug")).unwrap();
    std::fs::create_dir_all(cache.join("out")).unwrap();
    let tag = "Signature: 8a477f597d28d172789f06886806bc55\n";
    std::fs::write(cache.join("target/CACHEDIR.TAG"), tag).unwrap();
    std::fs::write(cache.join("target/debug/bin"), [0; 4096]).unwrap();
    // Ignored like `target/`, but untagged: nothing says it is safe to delete.
    std::fs::write(cache.join("out/report"), [0; 100]).unwrap();
    std::fs::write(tmp.join("wt-dirty/new.txt"), "x").unwrap();
    // The record is gone (pruned, or the repository moved); the directory stays.
    std::fs::remove_dir_all(work.join(".git/worktrees/wt-orphan")).unwrap();

    // From a linked worktree: git resolves the repository, whichever checkout we stand in.
    let rows = json(&cache, &["worktree", "list", "--json"]);
    let row = |name: &str| by_name(&rows, name);
    assert_eq!(rows.as_array().unwrap().len(), 4, "{rows}");
    assert_eq!(row("work")["state"], "main");
    assert_eq!(row("work")["cache_bytes"], 0);
    assert_eq!(row("wt-dirty")["state"], "dirty");

    let cached = row("wt-cache");
    assert_eq!(cached["state"], "unmerged");
    assert_eq!(cached["branch"], "t-cache");
    assert_eq!(cached["cache_bytes"], 4096 + tag.len() as u64);
    let source = cached["source_bytes"].as_u64().unwrap();
    assert!((100..4096).contains(&source), "{source}");
    assert!(cached["modified_unix"].as_u64().unwrap() > 0);

    let orphan = row("wt-orphan");
    assert_eq!(orphan["state"], "orphan");
    assert!(orphan["branch"].is_null() && orphan["owner"].is_null());

    let table = rtok(&work, &["worktree", "list"]);
    let table = String::from_utf8_lossy(&table.stdout);
    assert!(table.starts_with("path "), "{table}");
    assert!(table.contains(" orphan "), "{table}");
    assert!(table.contains("4 worktrees: "), "{table}");

    let outside = rtok(&tmp, &["worktree", "list"]);
    assert!(!outside.status.success());
    let err = String::from_utf8_lossy(&outside.stderr);
    assert!(err.contains("not a git repository"), "{err}");
}

/// T154: with no lock reason, `list` names the session the hooks saw working in a worktree —
/// one `SessionStart` is enough, a second one from the same session adds no second owner,
/// and the main checkout belongs to nobody. The lock reason still wins where there is one.
#[test]
fn list_names_the_session_the_hooks_saw_in_an_unlocked_worktree() {
    let tmp = rtok::testutil::tmp_dir("worktree-seen");
    run(&tmp, &["init", "-q", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    add(&work, "locked", Some(&format!("{ME} | t1 | 2026-09-22")));
    add(&work, "free", None);
    add(&work, "idle", None);
    let free = tmp.join("wt-free");
    let hook = |cwd: &Path, session: &str| {
        let stdin = serde_json::json!({
            "hook_event_name": "SessionStart", "session_id": session,
            "cwd": cwd, "source": "startup",
        });
        let out = rtok_in(
            &tmp,
            cwd,
            &["hook", "SessionStart"],
            stdin.to_string().as_bytes(),
        );
        assert!(out.status.success(), "hooks exit 0");
    };
    hook(&free, "sess-free-1");
    hook(&free, "sess-free-1");
    hook(&work, "sess-main");
    hook(&tmp.join("wt-locked"), "sess-locked");

    let rows = json_in(&tmp, &work, &["worktree", "list", "--json"]);
    let row = |name: &str| by_name(&rows, name);
    assert!(row("work")["session"].is_null(), "{rows}");
    assert!(row("wt-idle")["session"].is_null(), "{rows}");
    let seen = &row("wt-free")["session"];
    assert_eq!(seen["session"], "sess-free-1", "{rows}");
    assert_eq!(seen["host"], "claude");
    assert!(seen["seen_unix"].as_i64().unwrap() > 0 && seen["live"] == true);
    assert_eq!(row("wt-locked")["owner"], ME);
    assert_eq!(row("wt-locked")["session"]["session"], "sess-locked");

    let table = rtok_in(&tmp, &work, &["worktree", "list"], b"");
    let table = String::from_utf8_lossy(&table.stdout);
    assert!(table.contains("seen claude sess-fre"), "{table}");
    assert!(table.contains(ME), "{table}");
}

/// T232: the Worktrees page renders the same table `rtok worktree list` prints — a
/// locked worktree's owner shows up on the page. The Worktrees page has no
/// config-driven root (like `worktree list` itself, T151): it reads the current
/// directory, so this pins it the way `tests/graph_model.rs` pins the Graph page —
/// via cwd. nextest runs each test in its own process, so this does not leak into
/// another test's relative paths; a plain multi-threaded `cargo test` run of this
/// file would race here.
///
/// The walk behind the page is a background read like `hosts_page_text` (T231),
/// never the tick itself (T206) — this real repository's own `target/` takes tens
/// of seconds to walk, which is exactly why a tick must never block on it. So the
/// first read of a fresh process answers "reading worktrees…" and this polls, the
/// same shape `tests/hosts_model.rs` uses for "probing hosts…".
#[test]
fn worktrees_page_shows_a_locked_worktree_s_owner() {
    let tmp = rtok::testutil::tmp_dir("worktree-page");
    run(&tmp, &["init", "-q", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    add(&work, "locked", Some(&format!("{ME} | t1 | 2026-09-22")));

    let cfg = rtok::testutil::config_file_in(&tmp);
    let prev = std::env::current_dir().unwrap();
    std::env::set_current_dir(&work).unwrap();
    // The read runs `git worktree list` on a background thread; on a loaded machine (parallel
    // cargo builds, a full nextest run) it has taken over 10 s, so the bound is generous.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let page = loop {
        let page = rtok::web::model::snapshot(&cfg)
            .worktrees
            .expect("the current directory reads fine");
        if page != "reading worktrees…\n" {
            break page;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the worktrees read never landed"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    std::env::set_current_dir(prev).unwrap();

    assert!(page.contains(ME), "{page}");
    assert!(page.contains("t-locked"), "{page}");
    let _ = std::fs::remove_dir_all(&tmp);
}

const ME: &str = "Claude Code / sonnet";

fn add(work: &Path, name: &str, lock: Option<&str>) {
    let (branch, path) = (format!("t-{name}"), format!("../wt-{name}"));
    let mut args = vec!["worktree", "add", "-q"];
    match lock {
        Some("") => args.push("--lock"),
        Some(reason) => args.extend(["--lock", "--reason", reason]),
        None => {}
    }
    args.extend(["-b", &branch, &path]);
    run(work, &args);
}

fn squash(work: &Path, branch: &str) {
    run(work, &["merge", "-q", "--squash", branch]);
    run(work, &["commit", "-q", "-m", branch]);
}

/// T153: `rtok worktree gc` removes what is finished, drops the records of deleted
/// directories, and stops at every lock that is not the caller's.
#[test]
fn gc_removes_only_finished_worktrees_and_never_opens_a_foreign_lock() {
    let tmp = rtok::testutil::tmp_dir("worktree-gc");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "work"]);
    let work = tmp.join("work");
    std::fs::write(work.join("list.txt"), "1\n2\n3\n").unwrap();
    run(&work, &["add", "list.txt"]);
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);

    let mine = format!("{ME} | t1 | 2026-09-22");
    let theirs = "Cursor / grok | t2 | 2026-09-22";
    let plain = ["done", "adj", "dirty", "open", "gone"];
    plain.iter().for_each(|name| add(&work, name, None));
    add(&work, "mine", Some(&mine));
    add(&work, "gone-mine", Some(&mine));
    add(&work, "theirs", Some(theirs));
    add(&work, "gone-theirs", Some(theirs));
    add(&work, "bare-lock", Some(""));

    commit(&tmp.join("wt-done"), "done.txt");
    run(&tmp.join("wt-done"), &["push", "-q", "origin", "t-done"]);
    commit(&tmp.join("wt-mine"), "mine.txt");
    commit(&tmp.join("wt-open"), "open.txt");
    std::fs::write(tmp.join("wt-dirty/new.txt"), "x").unwrap();
    std::fs::write(tmp.join("wt-adj/list.txt"), "1\n2\n3\nx\n").unwrap();
    run(&tmp.join("wt-adj"), &["commit", "-q", "-am", "x"]);
    for branch in ["t-done", "t-mine", "t-adj"] {
        squash(&work, branch);
    }
    // The base moves on next to the merged lines: the trial merge now conflicts, and
    // only the patch-equivalence signal still sees the squash.
    std::fs::write(work.join("list.txt"), "1\n2\n3\nx\ny\n").unwrap();
    run(&work, &["commit", "-q", "-am", "y"]);
    run(&work, &["push", "-q", "origin", "main"]);
    for name in ["gone", "gone-mine", "gone-theirs"] {
        std::fs::remove_dir_all(tmp.join(format!("wt-{name}"))).unwrap();
    }
    let listed = || run(&work, &["worktree", "list", "--porcelain"]);
    let before = listed();

    // Dry run from inside a finished worktree: a full plan, and nothing changes. The store
    // lives under `tmp` — one under a worktree would make it dirty (T285: gc reads agents).
    let idle0 = ["worktree", "gc", "--json", "--owner", ME, "--idle", "0h"];
    let plan = json_in(&tmp, &tmp.join("wt-done"), &idle0);
    let planned = |name: &str| {
        let row = by_name(&plan, name);
        let (action, note) = (
            row["action"].as_str().unwrap(),
            row["note"].as_str().unwrap(),
        );
        format!("{action}: {note}")
    };
    assert_eq!(planned("work"), "keep: main checkout");
    assert_eq!(
        planned("wt-done"),
        "keep: the worktree this command runs from"
    );
    assert_eq!(planned("wt-mine"), "remove: merged, clean and idle");
    assert_eq!(planned("wt-adj"), "remove: merged, clean and idle");
    assert_eq!(planned("wt-dirty"), "keep: uncommitted changes");
    let open = "keep: not merged into the base; check `gh pr view t-open`";
    assert_eq!(planned("wt-open"), open);
    assert_eq!(planned("wt-theirs"), "keep: locked by Cursor / grok");
    assert_eq!(planned("wt-bare-lock"), "keep: locked, owner unknown");
    assert_eq!(planned("wt-gone"), "drop-record: directory is gone");
    assert_eq!(planned("wt-gone-mine"), "drop-record: directory is gone");
    assert_eq!(planned("wt-gone-theirs"), "keep: locked by Cursor / grok");
    assert_eq!(listed(), before);

    // No `--owner`, default idle window: only the unlocked stale record may go.
    let cautious = json_in(&tmp, &work, &["worktree", "gc", "--json", "--yes"]);
    let note = |rows, name: &str| by_name(rows, name)["note"].as_str().unwrap().to_owned();
    assert_eq!(
        note(&cautious, "wt-done"),
        "modified within the idle window"
    );
    assert_eq!(note(&cautious, "wt-mine"), format!("locked by {ME}"));
    assert_eq!(note(&cautious, "wt-gone-mine"), format!("locked by {ME}"));
    assert_eq!(note(&cautious, "wt-gone"), "removed with its branch");

    let applied = json_in(&tmp, &work, &[&idle0[..], &["--yes"]].concat());
    let remote_left = "removed with its branch; remote left: git push origin --delete t-done";
    assert_eq!(note(&applied, "wt-done"), remote_left);
    let kept = [
        "work",
        "wt-dirty",
        "wt-open",
        "wt-theirs",
        "wt-bare-lock",
        "wt-gone-theirs",
    ];
    let after = listed();
    let paths: Vec<&str> = after
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .collect();
    assert_eq!(paths.len(), kept.len(), "{after}");
    assert!(
        kept.iter().all(|k| paths.iter().any(|p| p.ends_with(k))),
        "{after}"
    );
    let branches = run(&work, &["branch", "--format=%(refname:short)"]);
    let mut branches: Vec<&str> = branches.lines().collect();
    branches.sort_unstable();
    let survivors = [
        "main",
        "t-bare-lock",
        "t-dirty",
        "t-gone-theirs",
        "t-open",
        "t-theirs",
    ];
    assert_eq!(branches, survivors);
    // Every admin entry left belongs to a directory, or to a lock gc may not open.
    let admin = std::fs::read_dir(work.join(".git/worktrees")).unwrap();
    let mut admin: Vec<String> = admin
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    admin.sort_unstable();
    assert_eq!(
        admin,
        [
            "wt-bare-lock",
            "wt-dirty",
            "wt-gone-theirs",
            "wt-open",
            "wt-theirs"
        ]
    );
    assert!(after.contains(&format!("locked {theirs}")), "{after}");
}

/// T158: `rtok worktree add` — the path is the only stdout line, the lock reason
/// round-trips through T150's parser, the branch has no upstream, and a second `add`
/// for the same task touches nothing.
#[test]
fn add_creates_one_locked_worktree_per_task_from_a_fresh_base() {
    let tmp = rtok::testutil::tmp_dir("worktree-add");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "apps/rtok"]);
    let work = tmp.join("apps/rtok");
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);
    // Only on the remote: a stale local `origin/main` would branch from the wrong commit.
    run(&tmp, &["clone", "-q", "origin.git", "other"]);
    commit(&tmp.join("other"), "b.txt");
    run(&tmp.join("other"), &["push", "-q", "origin", "main"]);
    let tip = run(&tmp.join("other"), &["rev-parse", "HEAD"]);
    let root = tmp.join("_worktrees");
    std::fs::create_dir_all(&root).unwrap();

    let owner = "Claude Code / sonnet";
    let out = rtok(
        &work,
        &["worktree", "add", "T158", "Worktree-Add", "--owner", owner],
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    let printed = String::from_utf8_lossy(&out.stdout);
    let path = Path::new(printed.trim_end());
    assert_eq!(printed.lines().count(), 1, "{printed}");
    // git reports the main checkout canonicalized (`/private/var` on macOS), and the
    // root follows it.
    let expected = root.join("rtok-t158").canonicalize().unwrap();
    assert_eq!(path.canonicalize().unwrap(), expected);
    assert_eq!(run(path, &["rev-parse", "HEAD"]), tip);
    assert_eq!(
        run(path, &["branch", "--show-current"]).trim(),
        "t158-worktree-add"
    );
    let upstream = Command::new("git")
        .args([
            "-C",
            printed.trim_end(),
            "rev-parse",
            "--abbrev-ref",
            "@{upstream}",
        ])
        .output()
        .unwrap();
    assert!(
        !upstream.status.success(),
        "the new branch must have no upstream"
    );

    let entries = inventory(&work).unwrap();
    let added = find(&entries, "rtok-t158");
    let parsed = added.record.owner().expect("the lock reason parses");
    assert_eq!(
        (parsed.owner.as_str(), parsed.task.as_str()),
        (owner, "t158")
    );
    assert!(!added.record.held_against(Some(owner)));

    let again = rtok(&work, &["worktree", "add", "t158", "--owner", owner]);
    assert!(!again.status.success());
    let err = String::from_utf8_lossy(&again.stderr);
    assert!(err.contains("one worktree per task"), "{err}");
    assert_eq!(inventory(&work).unwrap().len(), 2);

    // `[worktree] root` wins over discovery; `~` expands.
    let home = tmp.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("config.toml"), "[worktree]\nroot = \"~/wt\"\n").unwrap();
    let cfg = home.join("config.toml").display().to_string();
    let out = rtok(
        &work,
        &["--config", &cfg, "worktree", "add", "t2", "--owner", owner],
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    let printed = String::from_utf8_lossy(&out.stdout);
    // Components, not a string suffix: Windows prints `\wt\rtok-t2`.
    let printed = Path::new(printed.trim_end());
    assert!(printed.ends_with("wt/rtok-t2"), "{}", printed.display());
}

/// T152: `rtok worktree clean` deletes idle tagged caches — and nothing else.
#[test]
fn clean_deletes_idle_tagged_caches_and_nothing_else() {
    let tmp = rtok::testutil::tmp_dir("worktree-clean");
    run(&tmp, &["init", "-q", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    std::fs::write(work.join(".git/info/exclude"), "target/\nout/\n").unwrap();
    for name in ["idle", "fresh", "orphan"] {
        add(&work, name, None);
    }
    std::fs::remove_dir_all(work.join(".git/worktrees/wt-orphan")).unwrap();
    let tag = "Signature: 8a477f597d28d172789f06886806bc55\n";
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 86_400);
    let cache = |dir: &Path, stale: bool| {
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        let files = ["target/CACHEDIR.TAG", "target/debug/bin"];
        std::fs::write(dir.join(files[0]), tag).unwrap();
        std::fs::write(dir.join(files[1]), [0; 4096]).unwrap();
        for file in files.iter().filter(|_| stale) {
            let file = std::fs::File::options().write(true).open(dir.join(file));
            file.unwrap().set_modified(old).unwrap();
        }
    };
    let idle = tmp.join("wt-idle");
    cache(&idle, true);
    cache(&tmp.join("wt-fresh"), false);
    cache(&tmp.join("wt-orphan"), true);
    cache(&work, true);
    // Same name, no tag: never touched.
    std::fs::create_dir_all(idle.join("out")).unwrap();
    std::fs::write(idle.join("out/report"), [0; 100]).unwrap();
    let bytes = 4096 + tag.len() as u64;

    // Dry run from the main checkout: its own cache is skipped, nothing is deleted.
    let rows = json(&work, &["worktree", "clean", "--json"]);
    let row = |name: &str| by_name(&rows, name);
    assert_eq!(rows.as_array().unwrap().len(), 4, "{rows}");
    assert_eq!(row("work")["action"], "keep");
    assert!(row("work")["note"].as_str().unwrap().contains("runs from"));
    assert_eq!(row("wt-idle")["action"], "clean");
    assert_eq!(
        (
            row("wt-idle")["cache"].as_str(),
            row("wt-idle")["bytes"].as_u64()
        ),
        (Some("target"), Some(bytes))
    );
    assert_eq!(row("wt-fresh")["action"], "keep");
    assert_eq!(row("wt-orphan")["action"], "clean");
    for dir in ["work", "wt-idle", "wt-fresh", "wt-orphan"] {
        assert!(tmp.join(dir).join("target/debug/bin").exists(), "{dir}");
    }
    let table = rtok(&work, &["worktree", "clean"]);
    let table = String::from_utf8_lossy(&table.stdout);
    assert!(table.contains("dry run: 2 caches, "), "{table}");

    let out = rtok(&work, &["worktree", "clean", "--yes"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!idle.join("target").exists());
    assert!(!tmp.join("wt-orphan/target").exists());
    assert!(tmp.join("wt-fresh/target/debug/bin").exists());
    assert!(work.join("target/debug/bin").exists());
    assert!(idle.join("out/report").exists() && idle.join("a.txt").exists());

    // Named: the current worktree goes too; a stranger to the repository is refused.
    let out = rtok(&work, &["worktree", "clean", "--yes", "."]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!work.join("target").exists() && work.join("a.txt").exists());
    let out = rtok(&work, &["worktree", "clean", "--yes", ".."]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not a worktree of this repository"), "{err}");
    assert!(tmp.join("wt-fresh/target/debug/bin").exists());
}

/// T285: the store `rtok` opens under `home`, with one fake agent row per host session.
fn agents(home: &Path, sessions: &[&str]) -> (rtok::store::Store, Vec<String>) {
    std::fs::create_dir_all(home.join(".rtok")).unwrap();
    let store = rtok::store::Store::open(&home.join(".rtok/rtok.db")).unwrap();
    let claude = store.host_id("claude").unwrap().unwrap();
    let ids = sessions
        .iter()
        .map(|s| store.register_agent(claude, s, None, None, None));
    let ids = ids.collect::<Result<Vec<_>, _>>().unwrap();
    (store, ids)
}

fn lock_of(work: &Path, name: &str) -> rtok::worktree::Owner {
    let entries = inventory(work).unwrap();
    find(&entries, name)
        .record
        .owner()
        .expect("the lock parses")
}

/// T285: `add` binds the worktree to the calling agent — `RTOK_AGENT_ID` or `--agent` — in
/// a v2 lock and a claim row; the owner defaults to the agent's host; no agent, no owner is
/// an error.
#[test]
fn add_binds_the_worktree_to_the_calling_agent() {
    let tmp = rtok::testutil::tmp_dir("worktree-add-agent");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "rtok"]);
    let work = tmp.join("rtok");
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);
    std::fs::create_dir_all(tmp.join("_worktrees")).unwrap();
    let (store, ids) = agents(&tmp, &["sess-add"]);
    let me = ids[0].as_str();

    let out = rtok_as(&tmp, &work, Some(me), &["worktree", "add", "t9"], b"");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let path = Path::new(std::str::from_utf8(&out.stdout).unwrap().trim_end());
    let lock = lock_of(&work, "rtok-t9");
    assert_eq!(
        (lock.owner.as_str(), lock.agent.as_deref()),
        ("claude", Some(me))
    );
    let claims = store.open_worktree_claims().unwrap();
    let real = path.canonicalize().unwrap().display().to_string();
    assert_eq!(claims, [(real, me.to_string())]);

    let args = ["worktree", "add", "t10", "--agent", &me[..8], "--owner", ME];
    let out = rtok_as(&tmp, &work, None, &args, b"");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lock = lock_of(&work, "rtok-t10");
    assert_eq!((lock.owner.as_str(), lock.agent.as_deref()), (ME, Some(me)));

    let bare = rtok_as(&tmp, &work, None, &["worktree", "add", "t11"], b"");
    assert!(String::from_utf8_lossy(&bare.stderr).contains("--owner is required"));
    let unknown = [
        "worktree", "add", "t11", "--agent", "ffffffff", "--owner", ME,
    ];
    let unknown = rtok_as(&tmp, &work, None, &unknown, b"");
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("--agent ffffffff: unknown"));
    assert_eq!(
        inventory(&work).unwrap().len(),
        3,
        "a refused add creates nothing"
    );
}

/// T285: `claim` takes an unlocked worktree or its own old lock, never another owner's or
/// another agent's.
#[test]
fn claim_takes_a_free_or_own_worktree_and_refuses_a_foreign_owner() {
    let tmp = rtok::testutil::tmp_dir("worktree-claim");
    run(&tmp, &["init", "-q", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    let (_store, ids) = agents(&tmp, &["sess-me", "sess-other"]);
    let (me, other) = (ids[0].as_str(), ids[1].as_str());
    add(&work, "free", None);
    add(&work, "old", Some("claude | t2 | 2026-09-22"));
    add(&work, "foreign", Some("Cursor / grok | t3 | 2026-09-22"));
    let theirs = format!("claude | t4 | 2026-09-22 | agent {other}");
    add(&work, "other", Some(&theirs));
    let claim = |name: &str| {
        let path = tmp.join(name).display().to_string();
        rtok_as(&tmp, &work, Some(me), &["worktree", "claim", &path], b"")
    };

    for (name, task) in [("wt-free", "t"), ("wt-old", "t2")] {
        let out = claim(name);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let lock = lock_of(&work, name);
        assert_eq!(
            (lock.task.as_str(), lock.agent.as_deref()),
            (task, Some(me))
        );
    }
    for (name, held) in [("wt-foreign", "Cursor / grok"), ("wt-other", other)] {
        let out = claim(name);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success() && err.contains("not taken"), "{err}");
        assert!(err.contains(held), "{err}");
    }
    assert_eq!(lock_of(&work, "wt-other").agent.as_deref(), Some(other));
    let main = claim("work");
    assert!(String::from_utf8_lossy(&main.stderr).contains("not a linked worktree"));
}

/// T285: `list --json` names the bound agent with its state — live, ended, none, or an old
/// lock without one — and `gc` keeps a live agent's merged worktree.
#[test]
fn list_shows_each_agent_s_state_and_gc_keeps_a_live_agent_s_worktree() {
    let tmp = rtok::testutil::tmp_dir("worktree-agents");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);
    let (store, ids) = agents(&tmp, &["sess-live", "sess-ended"]);
    store.end_agent(&ids[1], 1).unwrap();
    let v2 = |id: &str| format!("{ME} | t1 | 2026-09-22 | agent {id}");
    add(&work, "live", Some(&v2(&ids[0])));
    add(&work, "ended", Some(&v2(&ids[1])));
    add(&work, "unclaimed", None);
    add(&work, "old", Some(&format!("{ME} | t1 | 2026-09-22")));

    let rows = json_in(&tmp, &work, &["worktree", "list", "--json"]);
    let agent = |name: &str| by_name(&rows, name)["agent"].clone();
    let bound =
        |id: &str, state: &str| serde_json::json!({"id": id, "host": "claude", "state": state});
    assert_eq!(agent("wt-live"), bound(&ids[0], "live"));
    assert_eq!(agent("wt-ended"), bound(&ids[1], "ended"));
    assert!(
        agent("wt-unclaimed").is_null() && agent("wt-old").is_null(),
        "{rows}"
    );
    assert_eq!(by_name(&rows, "wt-old")["owner"], ME);
    let table = rtok_in(&tmp, &work, &["worktree", "list"], b"");
    let table = String::from_utf8_lossy(&table.stdout);
    assert!(table.contains("agent state"), "{table}");
    assert!(
        table.contains(&format!("{} claude live", &ids[0][..8])),
        "{table}"
    );

    let gc = ["worktree", "gc", "--json", "--owner", ME, "--idle", "0h"];
    let plan = json_in(&tmp, &work, &gc);
    let note = |name: &str| by_name(&plan, name)["note"].as_str().unwrap().to_owned();
    assert_eq!(note("wt-live"), format!("agent {} is live", &ids[0][..8]));
    assert_eq!(note("wt-ended"), "merged, clean and idle");
}

/// T286: `remove` takes the caller's own clean worktree — with its branch once merged, or
/// keeping an unmerged one on `--keep-branch` — releases the claim, and refuses a dirty
/// worktree, another agent's, and the one it runs from.
#[test]
fn remove_takes_only_the_caller_s_own_clean_worktree() {
    let tmp = rtok::testutil::tmp_dir("worktree-remove");
    run(&tmp, &["init", "-q", "--bare", "origin.git"]);
    run(&tmp, &["clone", "-q", "origin.git", "work"]);
    let work = tmp.join("work");
    commit(&work, "a.txt");
    run(&work, &["push", "-q", "-u", "origin", "main"]);
    run(&work, &["remote", "set-head", "origin", "main"]);
    let (store, ids) = agents(&tmp, &["sess-me", "sess-other"]);
    let (me, other) = (ids[0].as_str(), ids[1].as_str());
    let lock = |task: &str, agent: &str| format!("claude | {task} | 2026-09-27 | agent {agent}");
    add(&work, "done", Some(&lock("t1", me)));
    add(&work, "open", Some(&lock("t2", me)));
    add(&work, "dirty", Some(&lock("t3", me)));
    add(&work, "theirs", Some(&lock("t4", other)));
    add(&work, "here", None);
    commit(&tmp.join("wt-done"), "done.txt");
    commit(&tmp.join("wt-open"), "open.txt");
    std::fs::write(tmp.join("wt-dirty/new.txt"), "x").unwrap();
    squash(&work, "t-done");
    run(&work, &["push", "-q", "origin", "main"]);
    let done = tmp.join("wt-done").canonicalize().unwrap();
    store
        .claim_worktree(&done.display().to_string(), me, "t1")
        .unwrap();
    let remove = |cwd: &Path, args: &[&str]| {
        let args = [&["worktree", "remove"][..], args].concat();
        rtok_as(&tmp, cwd, Some(me), &args, b"")
    };
    let refused = |cwd: &Path, args: &[&str], why: &str| {
        let out = remove(cwd, args);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.code() == Some(1) && err.contains(why), "{err}");
    };
    let branch = |name: &str| !run(&work, &["branch", "--list", name]).is_empty();

    let out = remove(&work, &["t1"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("removed with its branch"), "{text}");
    assert!(!done.exists() && !branch("t-done"));
    assert!(store.open_worktree_claims().unwrap().is_empty());

    refused(&work, &["../wt-open"], "--keep-branch");
    assert!(tmp.join("wt-open").exists());
    let out = remove(&work, &["../wt-open", "--keep-branch", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let removed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(removed["note"], "removed; branch kept");
    assert!(!tmp.join("wt-open").exists() && branch("t-open"));

    refused(&work, &["t3"], "uncommitted or untracked");
    refused(&work, &["../wt-theirs"], other);
    refused(&tmp.join("wt-here"), &["."], "current directory");
    for name in ["wt-dirty", "wt-theirs", "wt-here"] {
        assert!(tmp.join(name).exists(), "{name}");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
