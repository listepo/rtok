//! T25.2: `rtok agent sessions` renders the operator model's Sessions page (T25.1) as a
//! table. Against a fixture store — two live sessions across two hosts and one ended —
//! this pins the Check: two rows by default, three with `--all`, the token columns sum to
//! exactly what `rtok stats` prints over the same window, the `agents` alias spells the
//! same command, and an empty store prints the header plus a line saying nothing is
//! running.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t252-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn rtok(args: &[&str], home: &Path) -> String {
    let out = Command::new(bin())
        .args(args)
        .env("RTOK_HOME", home)
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "rtok {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Two live sessions across the two hosts the migrations seed (`claude`, `pi`) and one
/// ended — the shape of the T25.1 store test, driven through the public API so the
/// command reads what the model reads. `live-a` spends (in, cc, cr, out) = (30, 1, 7, 7),
/// `gone-c` (7, 2, 0, 1), `live-b` nothing yet. The transcript files mirror the store's
/// `usage` rows turn for turn, so `rtok stats` (which counts transcripts) and
/// `agent sessions` (which sums `usage` rows) agree on this fixture — the same window,
/// both definitions of the numbers.
fn seed(home: &Path) {
    let projects = home.join(".claude/projects/acme");
    fs::create_dir_all(&projects).unwrap();
    fs::write(
        projects.join("live-a.jsonl"),
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"one\"}],\"usage\":{\"input_tokens\":10,\"cache_creation_input_tokens\":1,\"cache_read_input_tokens\":2,\"output_tokens\":3}}}\n\
         {\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"two\"}],\"usage\":{\"input_tokens\":20,\"cache_read_input_tokens\":5,\"output_tokens\":4}}}\n",
    )
    .unwrap();
    fs::write(
        projects.join("gone-c.jsonl"),
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"bye\"}],\"usage\":{\"input_tokens\":7,\"cache_creation_input_tokens\":2,\"output_tokens\":1}}}\n",
    )
    .unwrap();
    let cfg = rtok::config::Config::load_from(home).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    let claude = store.host_id("claude").unwrap().expect("0002 seeds claude");
    let pi = store.host_id("pi").unwrap().expect("0010 seeds pi");
    store
        .upsert_session("live-a", Some(claude), Some("rtok"), None, Some("proxy"))
        .unwrap();
    store
        .upsert_session("live-b", Some(pi), Some("rtok"), None, None)
        .unwrap();
    store
        .upsert_session("gone-c", Some(pi), None, None, None)
        .unwrap();
    let (pid, mid) = store.upsert_model("anthropic", "claude-x").unwrap();
    let call_a = store
        .insert_call(
            "live-a",
            "proxy",
            "api_request",
            Some(claude),
            Some(pid),
            Some(mid),
            None,
            Some("/v1/messages"),
        )
        .unwrap();
    let call_c = store
        .insert_call(
            "gone-c",
            "proxy",
            "api_request",
            Some(pi),
            None,
            None,
            None,
            Some("/v1/chat/completions"),
        )
        .unwrap();
    store
        .insert_usage("live-a", Some("claude-x"), "anthropic", 10, 1, 2, 3, call_a)
        .unwrap();
    store
        .insert_usage("live-a", Some("claude-x"), "anthropic", 20, 0, 5, 4, call_a)
        .unwrap();
    store
        .insert_usage("gone-c", Some("gpt-x"), "openai_chat", 7, 2, 0, 1, call_c)
        .unwrap();
    store
        .end_session("gone-c", rtok::log::now() as i64)
        .unwrap();
}

/// The four token columns of one data row: everything before the `started` column — the
/// only cell that contains a space — split on whitespace. The index comes from the
/// header, whose columns line up with the rows' by construction.
fn token_columns(line: &str, header: &str) -> Vec<i64> {
    let at = header.find("started").expect("header names started");
    line[..at]
        .split_whitespace()
        .skip(3) // agent, provider, model
        .map(|n| n.parse().expect("numbers before started"))
        .collect()
}

#[test]
fn two_live_and_one_ended_is_two_rows_three_with_all() {
    let h = home("rows");
    seed(&h);

    let out = rtok(&["agent", "sessions"], &h);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3, "header plus the two live rows:\n{out}");
    assert!(lines[0].starts_with("agent"), "header first:\n{out}");
    assert!(out.contains("claude") && out.contains("anthropic"), "{out}");
    assert!(
        !out.contains("gpt-x") && !out.contains("openai_chat"),
        "the ended session is hidden without --all:\n{out}"
    );

    let all = rtok(&["agent", "sessions", "--all"], &h);
    let all_lines: Vec<&str> = all.lines().collect();
    assert_eq!(all_lines.len(), 4, "the ended row joins:\n{all}");
    assert!(all.contains("gpt-x"), "{all}");

    // `agents` is the visible alias the request spelled (P25): same command, same bytes.
    assert_eq!(rtok(&["agents", "sessions", "--all"], &h), all);
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn the_token_columns_equal_rtok_stats_over_the_same_window() {
    let h = home("parity");
    seed(&h);

    let all = rtok(&["agent", "sessions", "--all"], &h);
    let mut lines = all.lines();
    let header = lines.next().unwrap().to_string();
    let mut sums = [0i64; 4];
    for line in lines {
        let cols = token_columns(line, &header);
        assert_eq!(cols.len(), 4, "in/out/cache_read/cache_create:\n{all}");
        for (sum, col) in sums.iter_mut().zip(cols) {
            *sum += col;
        }
    }
    // `stats`' `usage_*` counts the transcripts the fixture wrote to mirror the store's
    // `usage` rows — the same window `--all` (since = 0) lists, so the two definitions
    // of the numbers must agree on it.
    let stats: serde_json::Value =
        serde_json::from_str(&rtok(&["stats", "--json"], &h)).expect("stats json");
    assert_eq!(
        sums,
        [
            stats["usage_input"].as_i64().unwrap(),
            stats["usage_output"].as_i64().unwrap(),
            stats["usage_cache_read"].as_i64().unwrap(),
            stats["usage_cache_create"].as_i64().unwrap()
        ],
        "sessions' token columns vs stats' usage:\n{all}"
    );
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn an_empty_store_prints_the_header_and_nothing_is_running() {
    let h = home("empty");
    let out = rtok(&["agent", "sessions"], &h);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "header plus the line:\n{out}");
    assert!(lines[0].starts_with("agent"), "header first:\n{out}");
    assert_eq!(lines[1], "nothing is running");
    let _ = fs::remove_dir_all(&h);
}
