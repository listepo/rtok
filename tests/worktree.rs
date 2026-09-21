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
    let out = rtok(&cache, &["worktree", "list", "--json"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    let rows: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = |name: &str| {
        let found = rows.as_array().unwrap().iter().find(|r| {
            let path = r["path"].as_str().unwrap();
            Path::new(path).ends_with(name)
        });
        found.unwrap_or_else(|| panic!("{name} missing in {rows}"))
    };
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
