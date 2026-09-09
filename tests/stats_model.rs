//! T15.11: `rtok stats` renders the D23 model. This file pins the command's output against a
//! fixture store (two transcript sessions, `usage` rows on both APIs, one `Measurement`), so
//! the move of the query into `src/web/model.rs` cannot change a printed number — and neither
//! can the next one. The goldens were recorded from the pre-move binary on 2026-09-09 and are
//! byte-for-byte what it printed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_rtok")
}

fn home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtok-t1511-{name}-{}", std::process::id()));
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
{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"Bash\",\"input\":{\"command\":\"cd /tmp && git status\"}}],\"usage\":{\"input_tokens\":10,\"output_tokens\":1}}}
{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"t1\",\"content\":\"hello world!!\"}]}}
{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"done\"}],\"usage\":{\"input_tokens\":20,\"cache_read_input_tokens\":80,\"output_tokens\":2}}}
";

const S2: &str = "\
{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"id\":\"r1\",\"name\":\"Read\",\"input\":{\"path\":\"x.rs\"}}],\"usage\":{\"input_tokens\":5,\"output_tokens\":1}}}
{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"r1\",\"content\":\"fn main() {}\"}]}}
{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}],\"usage\":{\"input_tokens\":7,\"cache_creation_input_tokens\":3,\"output_tokens\":1}}}
";

/// Two transcript sessions, a store with two `usage` rows (both APIs) and one `Measurement`.
fn seed(home: &Path) {
    let projects = home.join(".claude/projects/acme");
    fs::create_dir_all(&projects).unwrap();
    fs::write(projects.join("s1.jsonl"), S1).unwrap();
    fs::write(projects.join("s2.jsonl"), S2).unwrap();

    let cfg = rtok::config::Config::load_from(home).expect("config");
    let store = rtok::store::Store::open(&cfg.core.db_path).expect("store");
    store
        .upsert_session("s1", None, None, None, Some("proxy"))
        .unwrap();
    let id1 = store
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
    let id2 = store
        .insert_call(
            "s1",
            "proxy",
            "api_request",
            None,
            None,
            None,
            None,
            Some("/v1/chat/completions"),
        )
        .unwrap();
    store
        .insert_usage("s1", Some("m"), "anthropic", 10, 1, 2, 3, id1)
        .unwrap();
    store
        .insert_usage("s1", Some("m"), "openai_chat", 20, 0, 5, 4, id2)
        .unwrap();
    store
        .insert_measurement(
            "s1",
            &rtok::Measurement {
                plugin: "cmd",
                kind: "filter",
                before_bytes: 100,
                after_bytes: 40,
                est_before: 25,
                est_after: 10,
                ref_id: None,
                call_id: None,
            },
        )
        .unwrap();
}

/// The table, byte for byte: `sessions` counts transcript files (two), the `api` rows come
/// from the store's `usage`, the tool rows from the transcripts.
#[test]
fn stats_table_is_unchanged_on_a_fixture_store() {
    let h = home("table");
    seed(&h);
    assert_eq!(
        rtok(&["stats"], &h),
        "\
sessions 2  lines 6  malformed 0
usage input=42 cache_create=3 cache_read=80 output=5  hit=64.0%  median_context=100
api                         input cache_create cache_read output    hit
anthropic                      10            1          2      3  15.4%
openai_chat                    20            0          5      4  20.0%
archive replay (estimate) ctt 14 → 14  -0.0%  over 0 results
tool                       count        bytes     mean      p95      max   est_tokens          ctt
Bash                           1           13       13       13       13            4            8
Read                           1           12       12       12       12            3            6
bash                       count        bytes     mean      p95      max   est_tokens          ctt
git                            1           13       13       13       13            4            8
mcp                        count        bytes     mean      p95      max   est_tokens          ctt
"
    );
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn stats_json_is_unchanged_on_a_fixture_store() {
    let h = home("json");
    seed(&h);
    assert_eq!(
        rtok(&["stats", "--json"], &h),
        r#"{
  "sessions": 2,
  "lines": 6,
  "malformed": 0,
  "tools": {
    "Bash": {
      "count": 1,
      "total_bytes": 13,
      "mean": 13,
      "p95": 13,
      "max": 13,
      "est_tokens": 4,
      "ctt": 8
    },
    "Read": {
      "count": 1,
      "total_bytes": 12,
      "mean": 12,
      "p95": 12,
      "max": 12,
      "est_tokens": 3,
      "ctt": 6
    }
  },
  "bash_families": {
    "git": {
      "count": 1,
      "total_bytes": 13,
      "mean": 13,
      "p95": 13,
      "max": 13,
      "est_tokens": 4,
      "ctt": 8
    }
  },
  "mcp_groups": {},
  "usage_input": 42,
  "usage_cache_create": 3,
  "usage_cache_read": 80,
  "usage_output": 5,
  "cache_hit_rate": 0.64,
  "median_final_context": 100,
  "ctt_total": 14,
  "ctt_archive": 14,
  "archive_candidates": 0,
  "api": {
    "anthropic": {
      "input": 10,
      "cache_create": 1,
      "cache_read": 2,
      "output": 3,
      "hit": 0.15384615384615385
    },
    "openai_chat": {
      "input": 20,
      "cache_create": 0,
      "cache_read": 5,
      "output": 4,
      "hit": 0.2
    }
  }
}"#
    );
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn stats_plugin_json_is_unchanged_on_a_fixture_store() {
    let h = home("plugin");
    seed(&h);
    assert_eq!(
        rtok(&["stats", "--plugin", "cmd"], &h),
        r#"{
  "archive_hits": 0,
  "plugin": "cmd",
  "rows": [
    {
      "after": 40,
      "before": 100,
      "est_after": 10,
      "est_before": 25,
      "kind": "filter",
      "ref_id": null
    }
  ]
}"#
    );
    let _ = fs::remove_dir_all(&h);
}

#[test]
fn stats_cache_table_is_unchanged_on_a_fixture_store() {
    let h = home("cache");
    seed(&h);
    assert_eq!(
        rtok(&["stats", "--cache"], &h),
        "\
session                                   turns   cache_read cache_create  busts
s1                                            2            7            1      0
"
    );
    let _ = fs::remove_dir_all(&h);
}
