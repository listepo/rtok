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
    Command::new(env!("CARGO_BIN_EXE_rtok"))
        .current_dir(cwd)
        .env("HOME", cwd)
        .args(args)
        .output()
        .expect("rtok runs")
}

/// A successful `rtok … --json` run, parsed.
fn json(cwd: &Path, args: &[&str]) -> serde_json::Value {
    let out = rtok(cwd, args);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "rtok {args:?}: {err}");
    serde_json::from_slice(&out.stdout).unwrap()
}

fn by_name<'a>(rows: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    let found = rows.as_array().unwrap().iter().find(|r| {
        let path = r["path"].as_str().unwrap();
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

    // Dry run from inside a finished worktree: a full plan, and nothing changes.
    let idle0 = ["worktree", "gc", "--json", "--owner", ME, "--idle", "0h"];
    let plan = json(&tmp.join("wt-done"), &idle0);
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
    let cautious = json(&work, &["worktree", "gc", "--json", "--yes"]);
    let note = |rows, name: &str| by_name(rows, name)["note"].as_str().unwrap().to_owned();
    assert_eq!(
        note(&cautious, "wt-done"),
        "modified within the idle window"
    );
    assert_eq!(note(&cautious, "wt-mine"), format!("locked by {ME}"));
    assert_eq!(note(&cautious, "wt-gone-mine"), format!("locked by {ME}"));
    assert_eq!(note(&cautious, "wt-gone"), "removed with its branch");

    let applied = json(&work, &[&idle0[..], &["--yes"]].concat());
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
