// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T329.2: `rtok graph projects` list / add / select / remove over a fixture store, with the
//! per-project index status `graph status` reports.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t3292-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn rtok(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rtok"))
        .args(args)
        .current_dir(home)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .expect("rtok runs")
}

fn ok(home: &Path, args: &[&str]) -> String {
    let out = rtok(home, args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn list(home: &Path) -> Vec<Value> {
    let text = ok(home, &["graph", "projects", "--json"]);
    serde_json::from_str::<Vec<Value>>(&text).unwrap()
}

fn by_name<'a>(rows: &'a [Value], name: &str) -> &'a Value {
    rows.iter().find(|r| r["name"] == name).unwrap()
}

#[test]
fn list_add_select_remove_round_trip_with_index_status() {
    let home = fixture("flow");
    let (a, b) = (home.join("alpha"), home.join("beta"));
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("lib.rs"), "fn used() {}\nfn main() { used(); }\n").unwrap();

    assert!(ok(&home, &["graph", "projects"]).contains("graph projects add"));
    ok(&home, &["graph", "projects", "add", a.to_str().unwrap()]);
    ok(&home, &["graph", "projects", "add", b.to_str().unwrap()]);
    ok(&home, &["graph", "index", a.to_str().unwrap()]);

    let rows = list(&home);
    assert_eq!(rows.len(), 2);
    let alpha = by_name(&rows, "alpha");
    assert_eq!(alpha["state"], "ok");
    assert_eq!(alpha["origin"], "manual");
    assert!(alpha["index"]["rows"].as_i64().unwrap() > 0);
    assert!(alpha["index"]["files"].as_i64().unwrap() > 0);
    assert_eq!(alpha["index"]["pending"], 0);
    assert_eq!(by_name(&rows, "beta")["state"], "not indexed");
    assert!(rows.iter().all(|r| r["selected"] == false));

    // Re-adding a known directory is a no-op on the registry.
    ok(&home, &["graph", "projects", "add", a.to_str().unwrap()]);
    assert_eq!(list(&home).len(), 2);

    let beta_id = by_name(&rows, "beta")["id"].as_i64().unwrap().to_string();
    ok(&home, &["graph", "projects", "select", &beta_id]);
    ok(&home, &["graph", "projects", "select", a.to_str().unwrap()]);
    let rows = list(&home);
    let selected: Vec<_> = rows.iter().filter(|r| r["selected"] == true).collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["name"], "alpha");
    let text = ok(&home, &["graph", "projects"]);
    assert!(
        text.lines()
            .any(|l| l.starts_with('*') && l.contains("alpha")),
        "{text}"
    );
    assert!(
        text.lines().all(|l| l == l.trim_end()),
        "no trailing pad: {text:?}"
    );

    fs::write(
        a.join("lib.rs"),
        "fn used() {}\nfn main() { used(); used(); }\n",
    )
    .unwrap();
    assert_eq!(by_name(&list(&home), "alpha")["state"], "stale");

    let gone = ok(&home, &["graph", "projects", "remove", &beta_id]);
    assert!(gone.contains("files untouched"), "{gone}");
    assert!(b.is_dir(), "remove never touches the project's files");
    assert_eq!(list(&home).len(), 1);
}

#[test]
fn bad_targets_fail_and_a_missing_root_is_listed_but_cannot_be_selected() {
    let home = fixture("bad");
    let a = home.join("alpha");
    fs::create_dir_all(&a).unwrap();
    let file = home.join("not-a-dir");
    fs::write(&file, "x").unwrap();

    assert!(
        !rtok(&home, &["graph", "projects", "add", file.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        !rtok(&home, &["graph", "projects", "select", "9999"])
            .status
            .success()
    );
    let err = rtok(&home, &["graph", "projects", "remove", "nowhere"]);
    assert!(String::from_utf8_lossy(&err.stderr).contains("no project"));

    ok(&home, &["graph", "projects", "add", a.to_str().unwrap()]);
    fs::remove_dir_all(&a).unwrap();
    let rows = list(&home);
    assert_eq!(rows[0]["state"], "missing");
    assert!(rows[0]["index"].is_null());
    assert!(
        !rtok(&home, &["graph", "projects", "select", "1"])
            .status
            .success()
    );
    assert!(ok(&home, &["graph", "projects"]).contains("missing"));
    ok(&home, &["graph", "projects", "remove", "1"]);
    assert!(list(&home).is_empty());
}
