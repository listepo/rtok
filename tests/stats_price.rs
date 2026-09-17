//! T49.1: `rtok stats --price` prices the proxy `usage` rows in USD.
//!
//! Fixture store: two priced models (`claude-sonnet-5`, `gpt-5-mini`), one
//! unknown model id and one `NULL`-model row. The table and the JSON pin the
//! exact rendering; the plain `stats` output must not mention costs at all.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t491-{name}-{}", std::process::id()));
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

const S1: &str = "\
{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}],\"usage\":{\"input_tokens\":4,\"output_tokens\":1}}}
";

/// Store rows priced by the shipped `[stats.prices]` defaults (sources in
/// `config/default.toml`): sonnet 2.0/2.5/0.2/10.0, mini 0.25/0.25/0.025/2.0.
/// Sonnet: 4.0 + 1.0 + 1.6 + 5.0 = $11.60, saved 8.0 × 1.8 = $14.40.
/// Mini: 1.0 + 0 + 0.1 + 2.0 = $3.10, saved 4.0 × 0.225 = $0.90.
/// Totals: $14.70 cost, $15.30 saved; the other two rows print `-`.
fn seed(home: &Path) {
    let projects = home.join(".claude/projects/acme");
    fs::create_dir_all(&projects).unwrap();
    fs::write(projects.join("s1.jsonl"), S1).unwrap();

    let cfg = rtok::config::Config::load_from(home).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    store
        .upsert_session("s1", None, None, None, Some("proxy"))
        .unwrap();
    let id = store
        .insert_call(
            "s1",
            "proxy",
            "api_request",
            None,
            None,
            None,
            None,
            Some("/v1/messages"),
        )
        .unwrap();
    store
        .insert_usage(
            "s1",
            Some("claude-sonnet-5"),
            "anthropic",
            2_000_000,
            400_000,
            8_000_000,
            500_000,
            id,
        )
        .unwrap();
    store
        .insert_usage(
            "s1",
            Some("gpt-5-mini"),
            "openai_chat",
            4_000_000,
            0,
            4_000_000,
            1_000_000,
            id,
        )
        .unwrap();
    store
        .insert_usage("s1", Some("mystery-1"), "anthropic", 30, 0, 0, 1, id)
        .unwrap();
    store
        .insert_usage("s1", None, "anthropic", 7, 0, 0, 0, id)
        .unwrap();
}

#[test]
fn stats_price_table_costs_priced_and_dashes_unknown() {
    let h = home("table");
    seed(&h);
    let out = rtok(&["stats", "--price"], &h);
    assert!(
        out.contains("cost total $14.70 (cache reads saved $15.30"),
        "{out}"
    );
    assert!(out.contains("claude-sonnet-5"), "{out}");
    assert!(out.contains("11.60"), "{out}");
    assert!(out.contains("gpt-5-mini"), "{out}");
    assert!(out.contains("3.10"), "{out}");
    for model in ["mystery-1", "unknown"] {
        let line = out
            .lines()
            .find(|l| l.starts_with(model))
            .unwrap_or_else(|| panic!("no row for {model}:\n{out}"));
        assert!(
            line.ends_with('-'),
            "unknown model must dash the money columns: {line}"
        );
    }
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn stats_price_json_carries_costs() {
    let h = home("json");
    seed(&h);
    let v: serde_json::Value =
        serde_json::from_str(&rtok(&["stats", "--price", "--json"], &h)).expect("stats json");
    let cost = &v["cost"];
    assert_eq!(cost["total_cost"], 14.7);
    assert_eq!(cost["total_saved"], 15.3);
    assert_eq!(cost["models"]["claude-sonnet-5"]["cost"], 11.6);
    assert_eq!(cost["models"]["claude-sonnet-5"]["saved"], 14.4);
    assert_eq!(cost["models"]["gpt-5-mini"]["cost"], 3.1);
    assert!(cost["models"]["mystery-1"]["cost"].is_null());
    assert_eq!(cost["models"]["mystery-1"]["input"], 30);
    assert_eq!(cost["unknown"], serde_json::json!(["mystery-1", "unknown"]));
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn stats_without_price_mentions_no_costs() {
    let h = home("plain");
    seed(&h);
    let table = rtok(&["stats"], &h);
    assert!(!table.contains("cost"), "{table}");
    let v: serde_json::Value =
        serde_json::from_str(&rtok(&["stats", "--json"], &h)).expect("stats json");
    assert!(v.get("cost").is_none(), "{v}");
    let _ = fs::remove_dir_all(&h);
}
